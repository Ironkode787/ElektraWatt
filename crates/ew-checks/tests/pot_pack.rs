// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-POT rules of WP-S1.16 part 2 over small drawings: one positive and one negative fixture per rule
//! (P2-1), and the note-once test of FT-188.

mod common;

use common::*;
use ew_checks::{Severity, Target};
use ew_derive::ew_schema::Kind;
use ew_model::Model;
use serde_json::{Value, json};

fn none() -> Value {
    json!({})
}

fn target(kind: Kind, n: u128) -> Target {
    Target::new(kind, uid(n))
}

/// A function definition with an FE-role CP 1.
const FNDEF_FE: u128 = 0x506;
const SYM_FE: u128 = 0x516;
/// A two-pole switch: opening pass-throughs 0→1 and 2→3, with the flags `flags`.
const FNDEF_2P: u128 = 0x507;
const SYM_2P: u128 = 0x517;
/// A supply: CPs 0 and 1 are source outputs (the flow starts at them).
const FNDEF_SUPPLY: u128 = 0x508;
const SYM_SUPPLY: u128 = 0x518;

/// [`Fx::base`] with the FE definition and a two-pole switch definition carrying `flags`.
fn base(flags: &Value) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        FNDEF_FE,
        SYM_FE,
        &[(0, "1", Value::Null), (1, "FE", json!({"potential_role": "fe"}))],
        &json!([]),
        &none(),
    );
    fx.fndef_with(
        FNDEF_2P,
        SYM_2P,
        &[
            (0, "1", Value::Null),
            (1, "2", Value::Null),
            (2, "3", Value::Null),
            (3, "4", Value::Null),
        ],
        &json!([
            {"a": 0, "b": 1, "kind": {"pass_through": {"opens": true}}},
            {"a": 2, "b": 3, "kind": {"pass_through": {"opens": true}}},
        ]),
        &json!({"flags": flags}),
    );
    fx.fndef_with(
        FNDEF_SUPPLY,
        SYM_SUPPLY,
        &[
            (0, "N", json!({"source_output": true})),
            (1, "L", json!({"source_output": true})),
        ],
        &json!([]),
        &none(),
    );
    fx
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-003

/// An FE-role CP wired to a plain function whose CP carries the definition `def`; `bonded` adds an FE–PE bond
/// at the FE CP.
fn fe_on(def: &Value, bonded: bool) -> Model {
    let mut fx = base(&none());
    fx.func_of(0x1001, FNDEF_FE, SYM_FE, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, Ep::Cp(0x1001, 1), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1002), def);
    if bonded {
        fx.func(0x1003, P1);
        fx.push(json!({
            "$k": "bond", "id": us(0x4001), "a": {"cp": [us(0x1001), 1]}, "b": {"cp": [us(0x1003), 0]},
            "kind": "fe_pe",
        }));
    }
    fx.build()
}

#[test]
fn pot003_reports_an_fe_pin_on_pe() {
    let f = findings_of(&fe_on(&json!({"ptype": "pe"}), false), "EW-POT-003");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005003");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "fe:1");
    assert_eq!(f[0].targets, vec![target(Kind::Function, 0x1001)]);
    assert!(f[0].message.contains("role FE"), "{}", f[0].message);
}

#[test]
fn pot003_an_fe_pin_on_fe_or_with_a_bond_is_silent() {
    assert!(findings_of(&fe_on(&json!({"ptype": "fe"}), false), "EW-POT-003").is_empty());
    assert!(findings_of(&fe_on(&json!({"ptype": "pe"}), true), "EW-POT-003").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-004, EW-POT-005

/// One wire 0x2001 with the colour override `colour` on a potential defined as `def`; `with_n` adds a separate
/// N potential to the project.
fn coloured(colour: &[&str], def: &Value, with_n: bool) -> Model {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.with("overrides", json!({"colour": colour}));
    fx.def(0x5001, cp(0x1001), def);
    if with_n {
        fx.func(0x1003, P1);
        fx.func(0x1004, P1);
        fx.conn(0x2002, P1, cp(0x1003), cp(0x1004), &none());
        fx.def(0x5002, cp(0x1003), &json!({"name": "N", "ptype": "n"}));
    }
    fx.build()
}

#[test]
fn pot004_reports_gnye_on_a_non_pe_potential() {
    let f = findings_of(
        &coloured(&["GN", "YE"], &json!({"ptype": "l"}), false),
        "EW-POT-004",
    );
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005004");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)]);
    assert!(f[0].message.contains("GNYE"), "{}", f[0].message);
}

#[test]
fn pot004_gnye_on_pe_is_silent() {
    let m = coloured(&["GN", "YE"], &json!({"ptype": "pe"}), false);
    assert!(findings_of(&m, "EW-POT-004").is_empty());
}

#[test]
fn pot005_reports_bu_on_a_non_n_conductor_in_a_project_with_n() {
    let f = findings_of(&coloured(&["BU"], &json!({"ptype": "m"}), true), "EW-POT-005");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005005");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)]);
}

#[test]
fn pot005_bu_without_a_neutral_in_the_project_or_on_n_is_silent() {
    let m = coloured(&["BU"], &json!({"ptype": "m"}), false);
    assert!(findings_of(&m, "EW-POT-005").is_empty());
    let m = coloured(&["BU"], &json!({"ptype": "n"}), true);
    assert!(findings_of(&m, "EW-POT-005").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-006

/// A device -K4097 with a 2.5 mm² line wire on CP 0 and a PE wire of `pe_csa` mm² on CP 1.
fn pe_sized(pe_csa: &str) -> Model {
    let mut fx = Fx::base();
    for n in [0x1001, 0x1002, 0x1003] {
        fx.func(n, P1);
    }
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.with("overrides", json!({"csa": "2.5"}));
    fx.conn(0x2002, P1, Ep::Cp(0x1001, 1), cp(0x1003), &none());
    fx.with("overrides", json!({"csa": pe_csa}));
    fx.def(0x5001, cp(0x1001), &json!({"name": "L1", "ptype": "l"}));
    fx.def(0x5002, Ep::Cp(0x1001, 1), &json!({"name": "PE", "ptype": "pe"}));
    fx.build()
}

#[test]
fn pot006_reports_an_undersized_pe_and_says_the_table_is_unverified() {
    let f = findings_of(&pe_sized("1.5"), "EW-POT-006");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005006");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2002)]);
    assert!(
        f[0].message
            .contains("below the 2.5 mm² required for the 2.5 mm² line conductors"),
        "{}",
        f[0].message
    );
    // P2-4: the shipped table is `verified = false`, and the finding says so.
    assert!(
        f[0].message
            .ends_with("(the sizing table is not verified against the standard)"),
        "{}",
        f[0].message
    );
}

#[test]
fn pot006_a_pe_as_large_as_the_line_is_silent() {
    assert!(findings_of(&pe_sized("2.5"), "EW-POT-006").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-008, PE-island half (FT-189)

const LOC: u128 = 0x900;

/// Two PE potentials (PE1 on -K4097/-K4098, PE2 on -K4099/-K4100) in a project whose one own location is +A;
/// `joined` wires them into one.
fn two_pe(joined: bool) -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(LOC), "aspect": "location", "code": "A"}));
    for n in [0x1001, 0x1002, 0x1003, 0x1004] {
        fx.func(n, P1);
    }
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2002, P1, cp(0x1003), cp(0x1004), &none());
    if joined {
        fx.conn(0x2003, P1, cp(0x1002), cp(0x1003), &none());
    }
    fx.def(0x5002, cp(0x1003), &json!({"name": "PE2", "ptype": "pe"}));
    fx.def(0x5001, cp(0x1001), &json!({"name": "PE1", "ptype": "pe"}));
    fx.build()
}

#[test]
fn pot008_reports_two_pe_potentials_in_one_location() {
    let f = findings_of(&two_pe(false), "EW-POT-008");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "island");
    assert_eq!(
        f[0].message,
        "+A holds 2 PE potentials: PE1, PE2; join them through a bonded rail or a PE wire"
    );
    assert_eq!(
        f[0].targets,
        vec![
            target(Kind::StructureNode, LOC),
            target(Kind::PotentialDef, 0x5001),
            target(Kind::PotentialDef, 0x5002),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
    );
    // Keyed on the location: the fingerprint survives a third island joining it.
    assert_eq!(
        f[0].fingerprint,
        ew_checks::Fingerprint::of("EW-POT-008", &[uid(LOC)], "island")
    );
}

#[test]
fn pot008_one_pe_potential_per_location_is_silent() {
    assert!(findings_of(&two_pe(true), "EW-POT-008").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-009

#[test]
fn pot009_reports_an_unconnected_cp_and_an_unused_one_as_info() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func_unused(0x1002, 1, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    let f = findings_of(&fx.build(), "EW-POT-009");
    assert_eq!(f.len(), 2, "{f:#?}");
    let warning = f
        .iter()
        .find(|x| x.targets == vec![target(Kind::Function, 0x1001)]);
    let warning = warning.expect("the open CP of -K4097");
    assert_eq!(warning.code, "005009");
    assert_eq!(warning.severity, Severity::Warning);
    assert_eq!(warning.discriminator, "1");
    assert_eq!(
        warning.message,
        "-1K4097:2 is not connected; connect it or mark it unused"
    );
    let info = f
        .iter()
        .find(|x| x.targets == vec![target(Kind::Function, 0x1002)]);
    let info = info.expect("the unused CP of -K4098");
    assert_eq!(info.severity, Severity::Info);
    assert!(info.message.contains("marked unused"), "{}", info.message);
}

#[test]
fn pot009_connected_cps_are_silent() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2002, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1002, 1), &none());
    assert!(findings_of(&fx.build(), "EW-POT-009").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-013

/// A two-pole switch 0x1003 (flags `flags`) whose pole 0→1 switches the potential defined as `def0` towards the
/// load -K4098 (CP 0), and whose pole 2→3, when `line_pole`, switches an L potential towards the same load (CP 1).
/// Without the line pole, the load's CP 1 is on an unswitched L potential.
fn switch(flags: &Value, def0: &Value, line_pole: bool) -> Model {
    let mut fx = base(flags);
    fx.func_of(0x1001, FNDEF_SUPPLY, SYM_SUPPLY, P1); // CP 0 N (or `def0`), CP 1 L
    fx.func(0x1002, P1); // the load
    fx.func_of(0x1003, FNDEF_2P, SYM_2P, P1);
    fx.conn(0x2001, P1, cp(0x1001), Ep::Cp(0x1003, 0), &none());
    fx.conn(0x2002, P1, Ep::Cp(0x1003, 1), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1001), def0);
    if line_pole {
        fx.conn(0x2003, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1003, 2), &none());
        fx.conn(0x2004, P1, Ep::Cp(0x1003, 3), Ep::Cp(0x1002, 1), &none());
    } else {
        fx.conn(0x2003, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1002, 1), &none());
    }
    fx.def(0x5002, Ep::Cp(0x1001, 1), &json!({"name": "L", "ptype": "l"}));
    fx.build()
}

fn n_def() -> Value {
    json!({"name": "N", "ptype": "n"})
}

#[test]
fn pot013_reports_a_switched_neutral_and_the_load_left_on_l() {
    let f = findings_of(&switch(&none(), &n_def(), false), "EW-POT-013");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005013");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "pair:0-1");
    assert_eq!(f[0].targets, vec![target(Kind::Function, 0x1003)]);
    assert!(
        f[0].message.contains("switches the neutral N → ") && f[0].message.contains("stays connected to L."),
        "{}",
        f[0].message
    );
}

#[test]
fn pot013_a_two_pole_switch_flagged_to_switch_n_with_l_is_silent() {
    let flags = json!({"switches_n_with_l": true});
    assert!(findings_of(&switch(&flags, &n_def(), true), "EW-POT-013").is_empty());
    // The flag without a line pole does not excuse it.
    assert_eq!(
        findings_of(&switch(&flags, &n_def(), false), "EW-POT-013").len(),
        1
    );
    // A switch on L is no N switching.
    let l = json!({"name": "L0", "ptype": "l"});
    assert!(findings_of(&switch(&none(), &l, false), "EW-POT-013").is_empty());
}

#[test]
fn pot013_an_n_isolating_terminal_is_an_info() {
    let f = findings_of(
        &switch(&json!({"n_isolation": true}), &n_def(), false),
        "EW-POT-013",
    );
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].severity, Severity::Info);
    assert!(f[0].message.contains("N-isolating"), "{}", f[0].message);
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-015

#[test]
fn pot015_derived_wire_numbers_are_unique() {
    // The derivation gives every wire its own label; the duplicate case is the unit test
    // `duplicate_wire_numbers_are_one_finding_per_label` (stored numbers arrive with FR-NUM-020).
    let mut fx = Fx::base();
    for n in [0x1001, 0x1002, 0x1003] {
        fx.func(n, P1);
    }
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.conn(0x2002, P1, Ep::Cp(0x1002, 1), cp(0x1003), &none());
    assert!(findings_of(&fx.build(), "EW-POT-015").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-016

/// A cable core of 1 mm² carried by the connection 0x2001 between -K4097 and -K4098, with the cross-section
/// override `csa`.
fn core_override(csa: &str) -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "device", "id": us(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    fx.push(json!({"$k": "conductor", "id": us(0x6101), "cable": us(0x6000), "core": "1", "csa": "1"}));
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(
        0x2001,
        P1,
        cp(0x1001),
        cp(0x1002),
        &json!({"kind": {"core": us(0x6101)}, "overrides": {"csa": csa}}),
    );
    fx.build()
}

#[test]
fn pot016_reports_an_override_equal_to_the_value_below_it() {
    let f = findings_of(&core_override("1"), "EW-POT-016");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005016");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "csa");
    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)]);
    assert!(f[0].message.contains("remove the override"), "{}", f[0].message);
}

#[test]
fn pot016_an_override_that_changes_the_value_is_silent() {
    assert!(findings_of(&core_override("2.5"), "EW-POT-016").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-017

/// Two definitions named `a` and `b` on one wire.
fn named(a: &str, b: &str) -> Model {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1001), &json!({"name": a}));
    fx.def(0x5002, cp(0x1002), &json!({"name": b}));
    fx.build()
}

#[test]
fn pot017_reports_two_names_in_one_potential() {
    let f = findings_of(&named("P10", "P2"), "EW-POT-017");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "005017");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "names");
    assert_eq!(f[0].subject, "P2");
    assert_eq!(
        f[0].message,
        "the definitions of one potential name it P2, P10; the first in natural order (P2) applies"
    );
    assert_eq!(f[0].targets.len(), 2);
    // FT-188: the derivation note of the same fact is not reported again as a second finding.
    let all = run_online(&named("P10", "P2"));
    assert_eq!(
        all.iter().filter(|x| x.rule == "EW-POT-017").count(),
        1,
        "{all:#?}"
    );
}

#[test]
fn pot017_one_name_is_silent() {
    assert!(findings_of(&named("P", "P"), "EW-POT-017").is_empty());
}

fn run_online(m: &Model) -> Vec<ew_checks::Finding> {
    let q = ew_derive::Q::new(m);
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online).findings
}
