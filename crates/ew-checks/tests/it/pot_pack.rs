// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-POT rules of WP-S1.16 part 2 over small drawings: one positive and one negative fixture per rule
//! (P2-1), and the note-once test of FT-188.

use crate::common;

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
fn pot003_cases() {
    rule_table(
        "EW-POT-003",
        vec![
            Case::reports(
                "an FE pin on PE is reported",
                fe_on(&json!({"ptype": "pe"}), false),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005003", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].discriminator, "fe:1", "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Function, 0x1001)], "case {c}");
                    assert!(f[0].message.contains("role FE"), "case {c}: {}", f[0].message);
                },
            ),
            Case::silent("an FE pin on FE", fe_on(&json!({"ptype": "fe"}), false)),
            Case::silent(
                "an FE pin on PE with a bond",
                fe_on(&json!({"ptype": "pe"}), true),
            ),
        ],
    );
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
fn pot004_cases() {
    rule_table(
        "EW-POT-004",
        vec![
            Case::reports(
                "GNYE on a non-PE potential is reported",
                coloured(&["GN", "YE"], &json!({"ptype": "l"}), false),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005004", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)], "case {c}");
                    assert!(f[0].message.contains("GNYE"), "case {c}: {}", f[0].message);
                },
            ),
            Case::silent(
                "GNYE on PE",
                coloured(&["GN", "YE"], &json!({"ptype": "pe"}), false),
            ),
        ],
    );
}

/// The colour cases of EW-POT-005; the built-in BU of DC control wires is
/// `pot005_the_built_in_bu_of_a_dc_control_wire_is_silent`.
#[test]
fn pot005_cases() {
    let ac_line = json!({"ptype": "l", "voltage": "230", "env": {"ac_dc": "ac"}});
    rule_table(
        "EW-POT-005",
        vec![
            Case::reports(
                "BU on a non-N conductor in a project with N is reported",
                coloured(&["BU"], &json!({"ptype": "m"}), true),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005005", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)], "case {c}");
                },
            ),
            Case::silent(
                "BU without a neutral in the project",
                coloured(&["BU"], &json!({"ptype": "m"}), false),
            ),
            Case::silent("BU on N", coloured(&["BU"], &json!({"ptype": "n"}), true)),
            // BU the user sets on an AC line conductor (a wire override) is still reported.
            Case::reports(
                "BU set by the user on an AC line is reported",
                coloured(&["BU"], &ac_line, true),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)], "case {c}");
                },
            ),
        ],
    );
}

/// [`coloured`] with no colour set anywhere: the wire takes the built-in colour of its potential (doc 08 §4.3).
fn default_coloured(def: &Value) -> Model {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    fx.def(0x5001, cp(0x1001), def);
    fx.func(0x1003, P1);
    fx.func(0x1004, P1);
    fx.conn(0x2002, P1, cp(0x1003), cp(0x1004), &none());
    fx.def(0x5002, cp(0x1003), &json!({"name": "N", "ptype": "n"}));
    fx.build()
}

/// EW-POT-005 judges a colour the project chose, not the built-in default: 24 V and 0 V wires the program colours
/// BU (L+, M, a DC signal; EN 60204-1 13.2.4) raise nothing in a project with N, typed with or without AC/DC
/// (the cycle-35 play-test drew them from a power supply with no voltage or AC/DC set).
#[test]
fn pot005_the_built_in_bu_of_a_dc_control_wire_is_silent() {
    for def in [
        json!({"name": "24V", "ptype": "l_plus"}),
        json!({"name": "0V", "ptype": "m"}),
        json!({"name": "0V", "ptype": "m", "voltage": "24", "env": {"ac_dc": "dc"}}),
        json!({"name": "S1", "ptype": "signal", "env": {"ac_dc": "dc"}}),
    ] {
        let m = default_coloured(&def);
        let q = ew_derive::Q::new(&m);
        let wire = ew_derive::ew_ids::Id::<ew_derive::ew_schema::Connection>::from_uuid(
            ew_derive::ew_ids::Uuid::from_u128(0x2001),
        );
        let props = q.get::<ew_derive::ConnWireProps>(wire);
        let colour = props[0].1.colour.clone().expect("a colour");
        assert!(
            matches!(
                colour.value.codes(),
                [ew_derive::ew_schema::ColourCode::Iec(
                    ew_derive::ew_schema::IecColour::Bu
                )]
            ),
            "{def}: {colour:?}"
        );
        assert!(
            matches!(colour.source, ew_derive::PropSource::BuiltIn { .. }),
            "{def}: {colour:?}"
        );
        assert!(findings_of(&m, "EW-POT-005").is_empty(), "{def}");
    }
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
fn pot006_cases() {
    rule_table(
        "EW-POT-006",
        vec![
            Case::reports(
                "an undersized PE is reported and the table is called unverified",
                pe_sized("1.5"),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005006", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2002)], "case {c}");
                    assert!(
                        f[0].message
                            .contains("below the 2.5 mm² required for the 2.5 mm² line conductors"),
                        "case {c}: {}",
                        f[0].message
                    );
                    // P2-4: the shipped table is `verified = false`, and the finding says so.
                    assert!(
                        f[0].message
                            .ends_with("(the sizing table is not verified against the standard)"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("a PE as large as the line", pe_sized("2.5")),
        ],
    );
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
fn pot008_pe_cases() {
    rule_table(
        "EW-POT-008",
        vec![
            Case::reports(
                "two PE potentials in one location are reported",
                two_pe(false),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].discriminator, "island", "case {c}");
                    assert_eq!(
                        f[0].message,
                        "+A holds 2 PE potentials: PE1, PE2; join them through a bonded rail or a PE wire",
                        "case {c}"
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
                        .collect::<Vec<_>>(),
                        "case {c}"
                    );
                    // Keyed on the location: the fingerprint survives a third island joining it.
                    assert_eq!(
                        f[0].fingerprint,
                        ew_checks::Fingerprint::of("EW-POT-008", &[uid(LOC)], "island"),
                        "case {c}"
                    );
                },
            ),
            Case::silent("one PE potential per location", two_pe(true)),
        ],
    );
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-009

#[test]
fn pot009_cases() {
    let mut open = Fx::base();
    open.func(0x1001, P1);
    open.func_unused(0x1002, 1, P1);
    open.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    let mut connected = Fx::base();
    connected.func(0x1001, P1);
    connected.func(0x1002, P1);
    connected.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    connected.conn(0x2002, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1002, 1), &none());
    rule_table(
        "EW-POT-009",
        vec![
            Case::reports(
                "an unconnected CP is reported and an unused one is an info",
                open.build(),
                |c, f| {
                    assert_eq!(f.len(), 2, "case {c}: {f:#?}");
                    let warning = f
                        .iter()
                        .find(|x| x.targets == vec![target(Kind::Function, 0x1001)]);
                    let warning = warning.unwrap_or_else(|| panic!("case {c}: the open CP of -K4097"));
                    assert_eq!(warning.code, "005009", "case {c}");
                    assert_eq!(warning.severity, Severity::Warning, "case {c}");
                    assert_eq!(warning.discriminator, "1", "case {c}");
                    assert_eq!(
                        warning.message, "-1K4097:2 is not connected; connect it or mark it unused",
                        "case {c}"
                    );
                    let info = f
                        .iter()
                        .find(|x| x.targets == vec![target(Kind::Function, 0x1002)]);
                    let info = info.unwrap_or_else(|| panic!("case {c}: the unused CP of -K4098"));
                    assert_eq!(info.severity, Severity::Info, "case {c}");
                    assert!(
                        info.message.contains("marked unused"),
                        "case {c}: {}",
                        info.message
                    );
                },
            ),
            Case::silent("connected CPs", connected.build()),
        ],
    );
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
fn pot013_cases() {
    let flags = json!({"switches_n_with_l": true});
    rule_table(
        "EW-POT-013",
        vec![
            Case::reports(
                "a switched neutral and the load left on L are reported",
                switch(&none(), &n_def(), false),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005013", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    assert_eq!(f[0].discriminator, "pair:0-1", "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Function, 0x1003)], "case {c}");
                    assert!(
                        f[0].message.contains("switches the neutral N → ")
                            && f[0].message.contains("stays connected to L."),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent(
                "a two-pole switch flagged to switch N with L",
                switch(&flags, &n_def(), true),
            ),
            Case::reports(
                "the flag without a line pole does not excuse it",
                switch(&flags, &n_def(), false),
                |c, f| assert_eq!(f.len(), 1, "case {c}: {f:#?}"),
            ),
            Case::silent(
                "a switch on L is no N switching",
                switch(&none(), &json!({"name": "L0", "ptype": "l"}), false),
            ),
            Case::reports(
                "an N-isolating terminal is an info",
                switch(&json!({"n_isolation": true}), &n_def(), false),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].severity, Severity::Info, "case {c}");
                    assert!(f[0].message.contains("N-isolating"), "case {c}: {}", f[0].message);
                },
            ),
        ],
    );
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-015: the derivation gives every wire its own label, so a clean model raises nothing
// (`core::clean_model_has_no_finding_from_any_shipped_rule`); the duplicate case is the unit test
// `duplicate_wire_numbers_are_one_finding_per_label` (stored numbers arrive with FR-NUM-020).

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
fn pot016_cases() {
    rule_table(
        "EW-POT-016",
        vec![
            Case::reports(
                "an override equal to the value below it is reported",
                core_override("1"),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005016", "case {c}");
                    assert_eq!(f[0].severity, Severity::Info, "case {c}");
                    assert_eq!(f[0].discriminator, "csa", "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Connection, 0x2001)], "case {c}");
                    assert!(
                        f[0].message.contains("remove the override"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("an override that changes the value", core_override("2.5")),
        ],
    );
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
fn pot017_cases() {
    rule_table(
        "EW-POT-017",
        vec![
            Case::reports(
                "two names in one potential are reported",
                named("P10", "P2"),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005017", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].discriminator, "names", "case {c}");
                    assert_eq!(f[0].subject, "P2", "case {c}");
                    assert_eq!(
                        f[0].message,
                        "the definitions of one potential name it P2, P10; the first in natural order (P2) applies",
                        "case {c}"
                    );
                    assert_eq!(f[0].targets.len(), 2, "case {c}");
                },
            ),
            Case::silent("one name", named("P", "P")),
        ],
    );
    // FT-188: the derivation note of the same fact is not reported again as a second finding.
    let all = run_online(&named("P10", "P2"));
    assert_eq!(
        all.iter().filter(|x| x.rule == "EW-POT-017").count(),
        1,
        "case two names in one potential, all rules: {all:#?}"
    );
}

fn run_online(m: &Model) -> Vec<ew_checks::Finding> {
    let q = ew_derive::Q::new(m);
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online).findings
}

// ---------------------------------------------------------------------------------------------------------
// EW-POT-021

/// A DC supply whose CP 0 has role L+ and CP 1 role M (the starter PSU's outputs, no source-output flag).
const FNDEF_DC: u128 = 0x509;
const SYM_DC: u128 = 0x519;

/// Two coils (-K4098, -K4099) fed from the supply 0x1001's + (A1 of each on it); their A2s are joined by one wire.
/// `returned` links the A2 net to the supply's −; `fed` = false leaves out the supply (the A1 net has no source).
fn coils(returned: bool, fed: bool) -> Model {
    let mut fx = Fx::base();
    fx.fndef_with(
        FNDEF_DC,
        SYM_DC,
        &[
            (0, "+", json!({"potential_role": "l_plus"})),
            (1, "-", json!({"potential_role": "m"})),
        ],
        &json!([]),
        &none(),
    );
    fx.func(0x1002, P1);
    fx.func(0x1003, P1);
    fx.conn(0x2002, P1, cp(0x1002), cp(0x1003), &none());
    fx.conn(0x2003, P1, Ep::Cp(0x1002, 1), Ep::Cp(0x1003, 1), &none());
    if fed {
        fx.func_of(0x1001, FNDEF_DC, SYM_DC, P1);
        fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    }
    if returned {
        fx.conn(0x2004, P1, Ep::Cp(0x1003, 1), Ep::Cp(0x1001, 1), &none());
    }
    fx.build()
}

#[test]
fn pot021_cases() {
    rule_table(
        "EW-POT-021",
        vec![
            Case::reports(
                "two coils whose A2 net reaches no supply are reported, once per coil",
                coils(false, true),
                |c, f| {
                    assert_eq!(f.len(), 2, "case {c}: {f:#?}");
                    for (x, n) in f.iter().zip([0x1002, 0x1003]) {
                        assert_eq!(x.code, "005021", "case {c}");
                        assert_eq!(x.severity, Severity::Warning, "case {c}");
                        assert_eq!(x.discriminator, "1", "case {c}: the A2 side");
                        assert_eq!(x.targets, vec![target(Kind::Function, n)], "case {c}");
                    }
                    assert!(
                        f[0].message.starts_with("-1K4098:2 is on ")
                            && f[0].message.ends_with(
                                "which reaches no supply: the load is fed but not returned; connect it to the \
                                 return conductor (0 V, N)"
                            ),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("the A2 net linked to the supply's −", coils(true, true)),
            Case::silent("no supply at all: unfed, not unreturned", coils(false, false)),
        ],
    );
}
