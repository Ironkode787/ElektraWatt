// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules over small drawings: potential attribute and flow conflicts, junction feeds, rings,
//! physical-wire conflicts and rail-bonded terminals without a location.

use crate::common;

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
    // FT-587 (WP-S1.16 D9): every definition is a target; the fingerprint stays keyed on the potential's ID.
    assert_eq!(
        f[0].targets,
        vec![
            target(Kind::PotentialDef, 0x5001),
            target(Kind::PotentialDef, 0x5002)
        ]
    );
    assert_eq!(
        f[0].fingerprint,
        ew_checks::Fingerprint::of("EW-POT-001", &[uid(0x5001)], &f[0].discriminator)
    );
    assert_eq!(f[0].subject, "P");
    assert!(f[0].message.contains("voltage"), "{}", f[0].message);
}

/// Two definitions with the attributes `at_a` (at the function tagged -K1) and `at_b` (at -K2); `swap` swaps
/// the definition IDs and the function IDs, so the ID order runs against the printed order.
fn two_defs_permuted(at_a: &Value, at_b: &Value, swap: bool) -> Model {
    let (fa, fb, da, db) = if swap {
        (0x1002, 0x1001, 0x5002, 0x5001)
    } else {
        (0x1001, 0x1002, 0x5001, 0x5002)
    };
    let mut fx = Fx::base();
    fx.func_tagged(fa, 1, P1);
    fx.func_tagged(fb, 2, P1);
    fx.conn(0x2001, P1, cp(fa), cp(fb), &none());
    fx.def(db, cp(fb), at_b);
    fx.def(da, cp(fa), at_a);
    fx.build()
}

#[test]
fn ft587_definitions_rank_in_natural_order_not_in_id_order() {
    // The definition at -K1 is first in natural order in both ID orders: its voltage applies, the text names
    // the other definition by its printed key, and both definitions are targets.
    let mut messages = Vec::new();
    for swap in [false, true] {
        let m = two_defs_permuted(&json!({"voltage": "24"}), &json!({"voltage": "230"}), swap);
        let f = findings_of(&m, "EW-POT-001");
        assert_eq!(f.len(), 1, "swap {swap}: {f:#?}");
        assert_eq!(f[0].targets.len(), 2, "swap {swap}");
        assert!(
            f[0].message.contains("24 applies"),
            "swap {swap}: {}",
            f[0].message
        );
        assert!(
            !f[0].message.contains(&us(0x5001)) && !f[0].message.contains(&us(0x5002)),
            "swap {swap}: no ID in the text: {}",
            f[0].message
        );
        messages.push(f[0].message.clone());
        let q = ew_derive::Q::new(&m);
        let part = q.get::<ew_derive::PotentialPartition>(());
        let p = part
            .potential_at(ew_model::EndpointKey::Cp(uid(0x1001).cast(), 0))
            .expect("a potential");
        let attrs = q.get::<ew_derive::PotentialAttrsOf>(p.id);
        assert_eq!(
            attrs.attrs.voltage.as_ref().map(|a| a.value.to_string()),
            Some("24".to_string()),
            "swap {swap}: the first definition in natural order applies"
        );
    }
    assert_eq!(messages[0], messages[1]);
}

#[test]
fn ft587_the_first_name_in_natural_order_wins_whatever_the_ids() {
    // Names rank first: "P2" sorts before "P10" in natural order (and after it in byte order).
    for swap in [false, true] {
        let m = two_defs_permuted(
            &json!({"name": "P10", "voltage": "230"}),
            &json!({"name": "P2", "voltage": "24"}),
            swap,
        );
        let f = findings_of(&m, "EW-POT-001");
        assert_eq!(f.len(), 1, "swap {swap}: {f:#?}");
        assert_eq!(f[0].subject, "P2", "swap {swap}");
        assert!(
            f[0].message.contains("24 applies"),
            "swap {swap}: {}",
            f[0].message
        );
        let names = findings_of(&m, "EW-POT-017");
        assert_eq!(names.len(), 1, "swap {swap}: {names:#?}");
        assert_eq!(names[0].targets.len(), 2, "swap {swap}");
        assert!(
            names[0].message.contains("P2"),
            "swap {swap}: {}",
            names[0].message
        );
    }
}

/// A PE-role function tagged -K1 and an M-role function tagged -K2 on one wire, without a definition; `swap`
/// swaps their IDs; `extra` adds a plain function tagged -K9 to the wire.
fn role_conflict(swap: bool, extra: bool) -> (Model, u128) {
    let (pe, m) = if swap { (0x1002, 0x1001) } else { (0x1001, 0x1002) };
    let mut fx = Fx::base();
    fx.func_role_tagged(pe, true, 1, P1);
    fx.func_role_tagged(m, false, 2, P1);
    fx.conn(0x2001, P1, Ep::Cp(pe, 1), Ep::Cp(m, 1), &none());
    if extra {
        fx.func_tagged(0x1009, 9, P1);
        fx.conn(0x2002, P1, Ep::Cp(m, 1), cp(0x1009), &none());
    }
    (fx.build(), pe)
}

fn attrs_finding(m: &Model) -> ew_checks::Finding {
    let f: Vec<_> = findings_of(m, "EW-POT-001")
        .into_iter()
        .filter(|f| f.discriminator.starts_with("attrs"))
        .collect();
    assert_eq!(f.len(), 1, "{f:#?}");
    f[0].clone()
}

#[test]
fn ft588_an_undefined_potential_is_keyed_on_its_discriminator_member() {
    for swap in [false, true] {
        let (m, pe) = role_conflict(swap, false);
        let f = attrs_finding(&m);
        // The discriminator member is the first member by its printed key (-K1:…), whatever the IDs.
        assert_eq!(f.discriminator, format!("attrs@{}:1", us(pe)), "swap {swap}");
        assert_eq!(
            f.fingerprint,
            ew_checks::Fingerprint::of("EW-POT-001", &[uid(pe)], &f.discriminator),
            "swap {swap}: keyed on the discriminator member's function only"
        );
        assert_eq!(f.targets.len(), 2, "swap {swap}: every member is a target");
        // A new member of the potential keeps the fingerprint (a waiver keeps matching) and joins the targets.
        let (grown, _) = role_conflict(swap, true);
        let g = attrs_finding(&grown);
        assert_eq!(g.fingerprint, f.fingerprint, "swap {swap}");
        assert_eq!(g.targets.len(), 3, "swap {swap}");
    }
}

/// WP-S1.16 P5-3 with D8: a typed waiver of EW-POT-001 on an undefined potential still waives the finding after a
/// member joins the potential, because the fingerprint keys on the discriminator member; without the waiver the
/// finding is open.
#[test]
fn a_waiver_of_an_undefined_potential_survives_a_new_member() {
    let fp = attrs_finding(&role_conflict(false, false).0).fingerprint;
    for extra in [false, true] {
        let pe = 0x1001;
        let mut fx = Fx::base();
        fx.func_role_tagged(pe, true, 1, P1);
        fx.func_role_tagged(0x1002, false, 2, P1);
        fx.conn(0x2001, P1, Ep::Cp(pe, 1), Ep::Cp(0x1002, 1), &none());
        if extra {
            fx.func_tagged(0x1009, 9, P1);
            fx.conn(0x2002, P1, Ep::Cp(0x1002, 1), cp(0x1009), &none());
        }
        fx.push(json!({
            "$k": "waiver", "id": us(0x9001), "rule": "EW-POT-001", "target": {"finding": fp.to_string()},
            "reason": {"en-GB": "accepted for the test"}, "author": "engineer", "date": "2025-04-14"
        }));
        let m = fx.build();
        let report = ew_checks::run(
            &ew_derive::Q::new(&m),
            &ew_checks::FULL,
            ew_checks::CheckMode::Online,
        );
        let f: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule == "EW-POT-001" && f.discriminator.starts_with("attrs"))
            .collect();
        assert_eq!(f.len(), 1, "extra {extra}: {f:#?}");
        assert_eq!(
            f[0].state,
            ew_checks::FindingState::Waived { waiver: us(0x9001) },
            "extra {extra}: the waiver matches"
        );
        assert_eq!(f[0].targets.len(), if extra { 3 } else { 2 });
        assert!(
            !report.findings.iter().any(|f| f.rule == "EW-CORE-081"),
            "extra {extra}: the waiver matched a finding"
        );
    }
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

/// FT-587 (ID-677): the flow conflict names the pass-through function by its device tag, not by an ID, so the text
/// is the same whichever IDs the records carry.
#[test]
fn ft587_the_flow_conflict_names_the_pass_through_by_its_tag() {
    let mut messages = Vec::new();
    for (a, b, pass) in [(0x1001, 0x1002, 0x1003), (0x1003, 0x1001, 0x1002)] {
        let mut fx = Fx::base();
        fx.func_tagged(a, 1, P1);
        fx.func_tagged(b, 2, P1);
        fx.pass_tagged(pass, 7, P1);
        fx.conn(0x2001, P1, cp(a), Ep::Cp(pass, 0), &none());
        fx.conn(0x2002, P1, Ep::Cp(pass, 1), cp(b), &none());
        fx.def(0x5001, cp(a), &json!({"ptype": "l_plus"}));
        fx.def(0x5002, cp(b), &json!({"ptype": "n"}));
        let f = findings_of(&fx.build(), "EW-POT-001");
        assert_eq!(f.len(), 1, "{f:#?}");
        assert!(f[0].message.contains("K7"), "{}", f[0].message);
        for n in [0x1001, 0x1002, 0x1003] {
            assert!(
                !f[0].message.contains(&us(n)),
                "no ID in the text: {}",
                f[0].message
            );
        }
        messages.push(f[0].message.clone());
    }
    assert_eq!(messages[0], messages[1]);
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
fn pot011_cases() {
    rule_table(
        "EW-POT-011",
        vec![
            Case::reports(
                "an ambiguous junction feed is reported",
                two_junctions(&[0x2001, 0x2002], &[0x2001, 0x2003]),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005011", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    // A potential without a definition targets every member function, whatever their ID order.
                    assert_eq!(
                        f[0].targets,
                        vec![target(Kind::Function, 0x1001), target(Kind::Function, 0x1002)],
                        "case {c}"
                    );
                    assert!(!f[0].discriminator.is_empty(), "case {c}");
                    // FT-711: plain words from the Fluent pattern, never the derivation's arm-and-root text.
                    let p = &f[0].subject;
                    assert_eq!(
                        f[0].message,
                        format!(
                            "on {p} it is unclear which wire feeds a junction or a group of interruption points; \
                             draw one wire into it from the supply side"
                        ),
                        "case {c}"
                    );
                    assert_eq!(
                        ew_checks::messages::format(&f[0], "en-GB").as_deref(),
                        Some(f[0].message.as_str()),
                        "case {c}: the en-GB pattern renders the message"
                    );
                    let nl = ew_checks::messages::render(&f[0], "nl-NL");
                    assert!(
                        nl.starts_with(&format!("op {p} is niet duidelijk welke draad")),
                        "case {c}: {nl}"
                    );
                },
            ),
            Case::silent(
                "a consistent junction tree is silent",
                two_junctions(&[0x2002, 0x2001], &[0x2001, 0x2003]),
            ),
        ],
    );
}

#[test]
fn pot012_cases() {
    let mut ring_fx = Fx::base();
    for f in [0x1001, 0x1002, 0x1003] {
        ring_fx.func(f, P1);
    }
    ring_fx.conn(0x2002, P1, cp(0x1001), cp(0x1002), &none());
    ring_fx.conn(0x2003, P1, cp(0x1002), cp(0x1003), &none());
    ring_fx.conn(0x2001, P1, cp(0x1003), cp(0x1001), &none());
    let mut chain_fx = Fx::base();
    for f in [0x1001, 0x1002, 0x1003] {
        chain_fx.func(f, P1);
    }
    chain_fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &none());
    chain_fx.conn(0x2002, P1, cp(0x1002), cp(0x1003), &none());
    rule_table(
        "EW-POT-012",
        vec![
            Case::reports(
                "a ring is reported by its broken connection",
                ring_fx.build(),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005012", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    // The highest connection of the ring forms no wire; its ID is the discriminator.
                    assert_eq!(f[0].discriminator, us(0x2003), "case {c}");
                },
            ),
            Case::silent("a chain has no ring", chain_fx.build()),
        ],
    );
}

/// A cable with the conductors 0x6101 and 0x6102 and an interruption-point pair joining the functions
/// 0x1001 and 0x1002 through the connections 0x2001 and 0x2002, which carry `core1` and `core2`.
fn conflict_fixture(core1: u128, core2: u128) -> Model {
    conflict_fixture_bonded(core1, core2, false)
}

/// [`conflict_fixture`], with `bonded` a 0 V–PE bond from -K4097:1 to a second potential (function 0x1003 wired
/// to 0x1004), so that the conflicting wire lies in a bond closure of two potentials (FT-188).
fn conflict_fixture_bonded(core1: u128, core2: u128, bonded: bool) -> Model {
    let mut fx = Fx::base();
    if bonded {
        fx.func(0x1003, P1);
        fx.func(0x1004, P1);
        fx.conn(0x2003, P1, cp(0x1003), cp(0x1004), &none());
        fx.push(json!({
            "$k": "bond", "id": us(0x4001), "a": {"cp": [us(0x1001), 0]}, "b": {"cp": [us(0x1003), 0]},
            "kind": "pelv_0v_pe",
        }));
    }
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
fn pot014_cases() {
    rule_table(
        "EW-POT-014",
        vec![
            Case::reports(
                "constituents that name different conductors are reported",
                conflict_fixture(0x6101, 0x6102),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005014", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    // The wire is carried by the connection at the target point; its ID is the discriminator.
                    assert_eq!(f[0].discriminator, us(0x2002), "case {c}");
                    // Every member function: the two functions and the two interruption points.
                    assert_eq!(f[0].targets.len(), 4, "case {c}");
                    assert!(f[0].targets.iter().all(|t| t.kind == Kind::Function), "case {c}");
                },
            ),
            Case::silent(
                "agreeing constituents are silent",
                conflict_fixture(0x6102, 0x6102),
            ),
        ],
    );
}

#[test]
fn ft188_a_wire_note_in_a_bond_closure_is_reported_once() {
    // Both potentials of the bond closure read the closure's physical wires; the note belongs to the potential
    // of the wire, and the bonded potential does not report it again.
    let m = conflict_fixture_bonded(0x6101, 0x6102, true);
    let q = ew_derive::Q::new(&m);
    assert_eq!(
        q.get::<ew_derive::PotentialPartition>(()).potential_count(),
        2,
        "two potentials joined by the bond"
    );
    let f = findings_of(&m, "EW-POT-014");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, us(0x2002));
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

#[test]
fn pot008_location_cases() {
    rule_table(
        "EW-POT-008",
        vec![
            // Amended by cycle 33 (G-PP ruling #20): the warning stays in a project with two or more own locations.
            Case::reports(
                "a rail-bonded terminal without a location is reported",
                rail_model(None, Some(0x1001), true),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005008", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Device, 0x4001)], "case {c}");
                    assert!(f[0].message.contains("no location"), "case {c}: {}", f[0].message);
                    assert!(
                        f[0].message.contains("2 own locations"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent(
                "a located terminal has no finding",
                rail_model(Some(LOC), Some(0x1001), true),
            ),
        ],
    );
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
fn pot002_cases() {
    rule_table(
        "EW-POT-002",
        vec![
            Case::reports(
                "a pelv potential without a bond is reported",
                bonded(&pelv_m(), 0),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005002", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    assert_eq!(f[0].discriminator, "pelv", "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::PotentialDef, 0x5001)], "case {c}");
                    assert!(
                        f[0].message.contains("no 0 V–PE bond"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("a pelv potential with one bond", bonded(&pelv_m(), 1)),
            Case::reports(
                "a pelv potential with two bonds is reported",
                bonded(&pelv_m(), 2),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].discriminator, "pelv", "case {c}");
                    assert!(
                        f[0].message.contains("has 2 0 V–PE bonds"),
                        "case {c}: {}",
                        f[0].message
                    );
                    let bonds = f[0].targets.iter().filter(|t| t.kind == Kind::Bond).count();
                    assert_eq!(bonds, 2, "case {c}: {:?}", f[0].targets);
                },
            ),
            Case::reports(
                "a bonded selv potential is reported",
                bonded(&json!({"ptype": "m", "env": {"earthing": "selv"}}), 1),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    assert_eq!(f[0].discriminator, "selv", "case {c}");
                    assert!(
                        f[0].targets.contains(&target(Kind::Bond, 0x4001)),
                        "case {c}: {:?}",
                        f[0].targets
                    );
                },
            ),
            Case::silent(
                "a pelv potential that is not M type",
                bonded(&json!({"ptype": "l_plus", "env": {"earthing": "pelv"}}), 0),
            ),
        ],
    );
}

// ---------------------------------------------------------------------------------------------------------
// WP-S1.16 part 1 (D2, D3; FT-551): EW-POT-001's quick fix "Declare 0 V–PE bond (PELV)".

const M_FN: u128 = 0x1001;
const TERMINAL: u128 = 0x1002;
const PE_FN: u128 = 0x1003;
const W1: u128 = 0x2001;
const W27: u128 = 0x2027;
const PE_FN2: u128 = 0x1004;
const W30: u128 = 0x2030;

/// 0 V (the M-role CP of `M_FN`) and PE (the PE-role CPs of `PE_FN` and `PE_FN2`, joined by wire W30) joined
/// through a terminal: wire W1 from the 0 V CP to the terminal, wire W27 from the terminal to the PE CP; `bond` adds
/// the bond the fix declares. `(w1, w27)` are the IDs of W1 and W27.
fn zero_volt_through_a_terminal_with(bond: bool, (w1, w27): (u128, u128)) -> Model {
    let mut fx = Fx::base();
    fx.func_m(M_FN, P1);
    fx.func(TERMINAL, P1);
    fx.func_pe(PE_FN, P1);
    fx.func_pe(PE_FN2, P1);
    fx.conn(w1, P1, Ep::Cp(M_FN, 1), cp(TERMINAL), &none());
    fx.conn(w27, P1, cp(TERMINAL), Ep::Cp(PE_FN, 1), &none());
    fx.conn(W30, P1, Ep::Cp(PE_FN, 1), Ep::Cp(PE_FN2, 1), &none());
    if bond {
        fx.push(json!({
            "$k": "bond", "id": us(0x4001), "a": {"cp": [us(TERMINAL), 0]}, "b": {"cp": [us(PE_FN), 1]},
            "kind": "pelv_0v_pe", "via": us(w27),
        }));
    }
    fx.build()
}

fn zero_volt_through_a_terminal(bond: bool) -> Model {
    zero_volt_through_a_terminal_with(bond, (W1, W27))
}

#[test]
fn pot001_offers_the_0v_pe_bond_on_the_wire_at_the_pe_side() {
    // The two ID orders of the wires: the carrier follows the drawing's printed keys, not the IDs (ID-677).
    for (w1, w27) in [(W1, W27), (W27, W1)] {
        let f = findings_of(&zero_volt_through_a_terminal_with(false, (w1, w27)), "EW-POT-001");
        assert_eq!(f.len(), 1, "{f:#?}");
        assert_eq!(
            f[0].message, "member roles M meet Pe; the first in role order applies",
            "a finding with a fix keeps its text"
        );
        let fix = f[0].fix.as_ref().expect("the bond fix");
        assert_eq!(fix.label, "Declare 0 V–PE bond (PELV)");
        assert_eq!(fix.command, "bond.create");
        // W1 and W27 split 0 V from PE (W30 leaves a PE CP on each side); W27 has an end on the PE-role CP, so it
        // comes first (D3).
        assert_eq!(
            fix.params,
            json!({
                "a": {"cp": {"function": us(TERMINAL), "idx": 0}},
                "b": {"cp": {"function": us(PE_FN), "idx": 1}},
                "kind": "pelv_0v_pe",
                "via": us(w27),
            })
        );
    }
}

#[test]
fn the_declared_bond_keeps_0v_and_pe_apart_and_satisfies_pot002() {
    let m = zero_volt_through_a_terminal(true);
    let q = ew_derive::Q::new(&m);
    assert_eq!(
        q.get::<ew_derive::PotentialPartition>(()).potential_count(),
        2,
        "the bond's via is cut: 0 V and PE are two potentials"
    );
    assert!(findings_of(&m, "EW-POT-001").is_empty());
    assert!(
        findings_of(&m, "EW-POT-002").is_empty(),
        "the one 0 V–PE bond of the PELV potential"
    );
}

/// Two parallel wires between the 0 V and the PE CP: removing either splits nothing, so there is no bond carrier.
fn zero_volt_meets_pe_without_a_carrier() -> Model {
    let mut fx = Fx::base();
    fx.func_m(M_FN, P1);
    fx.func_pe(PE_FN, P1);
    fx.conn(W1, P1, Ep::Cp(M_FN, 1), Ep::Cp(PE_FN, 1), &none());
    fx.conn(W27, P1, Ep::Cp(M_FN, 1), Ep::Cp(PE_FN, 1), &none());
    fx.build()
}

#[test]
fn pot001_without_a_carrier_has_no_fix_and_names_draw_bond() {
    let f = findings_of(&zero_volt_meets_pe_without_a_carrier(), "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].fix.is_none(), "{:?}", f[0].fix);
    assert_eq!(
        f[0].message,
        "member roles M meet Pe; the first in role order applies; declare the bond with Draw ▸ Bond"
    );
}

#[test]
fn pot001_a_conflict_without_pe_has_no_fix_and_no_hint() {
    // M against L: a definition of type L on the 0 V CP's potential.
    let mut fx = Fx::base();
    fx.func_m(M_FN, P1);
    fx.func(TERMINAL, P1);
    fx.conn(W1, P1, Ep::Cp(M_FN, 1), cp(TERMINAL), &none());
    fx.def(0x5001, cp(TERMINAL), &json!({"ptype": "l"}));
    let f = findings_of(&fx.build(), "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].fix.is_none());
    assert!(!f[0].message.contains("Draw"), "{}", f[0].message);
}

/// A type-M definition on a terminal wired to `n` PE-role functions: one member-CP conflict each.
fn pe_members_against_m(n: u128) -> ew_checks::Finding {
    let mut fx = Fx::base();
    fx.func(TERMINAL, P1);
    fx.def(0x5001, cp(TERMINAL), &json!({"ptype": "m"}));
    for i in 0..n {
        fx.func_pe(0x1100 + i, P1);
        fx.conn(0x2100 + i, P1, cp(TERMINAL), Ep::Cp(0x1100 + i, 1), &none());
    }
    attrs_finding(&fx.build())
}

/// Cycle 37 (window check): EW-POT-001's message names at most three member CPs and then "and N more member CPs";
/// the finding's `members` argument keeps every conflict, and nl-NL renders the same cut from the arguments.
#[test]
fn pot001_names_three_member_cps_and_counts_the_rest() {
    let f = pe_members_against_m(6);
    assert_eq!(f.message.matches("member CP ").count(), 3, "{}", f.message);
    assert!(f.message.ends_with("; and 3 more member CPs"), "{}", f.message);
    let members = &f.args.iter().find(|(k, _)| *k == "members").expect("members").1;
    assert_eq!(members.matches("member CP ").count(), 6, "{members}");
    let nl = ew_checks::messages::render(&f, "nl-NL");
    assert!(nl.ends_with("; en nog 3 aansluitpunten"), "{nl}");
    assert_eq!(nl.matches("member CP ").count(), 3, "{nl}");
}

/// Three member CPs or fewer are all named, with no count.
#[test]
fn pot001_names_every_member_cp_up_to_three() {
    let f = pe_members_against_m(3);
    assert_eq!(f.message.matches("member CP ").count(), 3, "{}", f.message);
    assert!(!f.message.contains("more"), "{}", f.message);
}

/// FT-724: EW-POT-001's text is built in the rule and in the Fluent pattern; the en-GB pattern rendered from the
/// finding's arguments equals the rule's text for the "and N more" cut, the bond hint and a plain conflict.
#[test]
fn ft724_the_en_gb_pattern_equals_the_rule_text() {
    let bond = findings_of(&zero_volt_meets_pe_without_a_carrier(), "EW-POT-001");
    assert_eq!(bond.len(), 1, "{bond:#?}");
    let cases = [
        ("and N more", pe_members_against_m(6)),
        ("bond hint", bond[0].clone()),
        ("plain", pe_members_against_m(2)),
    ];
    for (case, f) in cases {
        assert_eq!(
            ew_checks::messages::format(&f, "en-GB").as_deref(),
            Some(f.message.as_str()),
            "{case}"
        );
    }
}
