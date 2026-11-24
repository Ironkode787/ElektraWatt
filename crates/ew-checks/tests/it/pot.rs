// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-POT rules over small drawings: potential attribute and flow conflicts, junction feeds, rings,
//! physical-wire conflicts and rail-bonded terminals without a location.

use crate::common;

use common::*;
use ew_checks::Severity;
use ew_derive::ew_schema::Kind;
use ew_model::Model;
use serde_json::{Value, json};

const STRIP: u128 = 0x400;
const LOC: u128 = 0x900;

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
    // Every definition is a target; the fingerprint stays keyed on the potential's ID.
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
    assert!(f[0].message().contains("voltage"), "{}", f[0].message());
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
fn definitions_rank_in_natural_order_not_in_id_order() {
    // The definition at -K1 is first in natural order in both ID orders: its voltage applies, the text names
    // the other definition by its printed key, and both definitions are targets.
    let mut messages = Vec::new();
    for swap in [false, true] {
        let m = two_defs_permuted(&json!({"voltage": "24"}), &json!({"voltage": "230"}), swap);
        let f = findings_of(&m, "EW-POT-001");
        assert_eq!(f.len(), 1, "swap {swap}: {f:#?}");
        assert_eq!(f[0].targets.len(), 2, "swap {swap}");
        assert!(
            f[0].message().contains("is wired to P at -1K1:1 (24 V)"),
            "swap {swap}: {}",
            f[0].message()
        );
        assert!(
            !f[0].message().contains(&us(0x5001)) && !f[0].message().contains(&us(0x5002)),
            "swap {swap}: no ID in the text: {}",
            f[0].message()
        );
        messages.push(f[0].message().clone());
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
fn the_first_name_in_natural_order_wins_whatever_the_ids() {
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
            f[0].message().contains("is wired to P2 at -1K2:1 (24 V)"),
            "swap {swap}: {}",
            f[0].message()
        );
        let names = findings_of(&m, "EW-POT-017");
        assert_eq!(names.len(), 1, "swap {swap}: {names:#?}");
        assert_eq!(names[0].targets.len(), 2, "swap {swap}");
        assert!(
            names[0].message().contains("P2"),
            "swap {swap}: {}",
            names[0].message()
        );
    }
}

/// A PE-role function tagged -K1 and an L+-role function tagged -K2 on one wire, without a definition; `swap`
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
fn an_undefined_potential_is_keyed_on_its_discriminator_member() {
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

/// A typed waiver of EW-POT-001 on an undefined potential still waives the finding after a
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

/// A PE-role CP and a CP of the role `live` (an L+ pin) or a 0 V pin on one wire, without a definition; `pe_first`
/// gives the PE function the lower ID. Returns the model and the IDs of the PE and the other function.
fn pe_meets(pe_first: bool, live: bool) -> (Model, u128, u128) {
    let (pe, other) = if pe_first {
        (0x1001, 0x1002)
    } else {
        (0x1002, 0x1001)
    };
    let mut fx = Fx::base();
    fx.func_pe(pe, P1);
    if live {
        fx.func_lplus(other, P1);
    } else {
        fx.func_m(other, P1);
    }
    fx.conn(0x2001, P1, Ep::Cp(pe, 1), Ep::Cp(other, 1), &none());
    (fx.build(), pe, other)
}

#[test]
fn pot001_member_role_conflict_does_not_depend_on_the_id_order() {
    // The two ID orders of one drawing (a new project mints random IDs): the applied role and the targets are the
    // same in both. Before the fix the CP (ID) order chose the applied role and the one target (the function of the
    // smallest CP), so a properties sheet of the PE terminal showed the finding in one copy of a project and not in
    // the other (blind replay gpp-2).
    for pe_first in [true, false] {
        let (m, pe, other) = pe_meets(pe_first, true);
        let f: Vec<_> = findings_of(&m, "EW-POT-001")
            .into_iter()
            .filter(|f| f.discriminator.starts_with("attrs"))
            .collect();
        assert_eq!(f.len(), 1, "pe_first {pe_first}: {f:#?}");
        let mut want = vec![target(Kind::Function, pe), target(Kind::Function, other)];
        want.sort();
        assert_eq!(
            f[0].targets, want,
            "pe_first {pe_first}: every member is a target"
        );
        // The live pin is named against the protective earth, in words, with the page and what to do.
        let other_tag = format!("-1K{other}:+");
        let pe_tag = format!("-1K{pe}:PE");
        assert_eq!(
            f[0].message(),
            format!(
                "{other_tag} (plus L+) is wired to {pe_tag} (protective earth PE) on page 1: a wire joins two \
                 different potentials, so one of the two pins is wired to the wrong conductor. Move that wire to the \
                 right one, or take it away."
            ),
            "pe_first {pe_first}"
        );
    }
}

/// A wire drawn from a 0 V pin straight to a protective-earth pin is the bond (doc 08 §3.7): in both ID orders no
/// EW-POT-001, the 0 V stays a potential of its own with the type M, the wire is a 0 V–PE bond of it, and EW-POT-002
/// counts it, so a PELV 0 V with that one wire is satisfied.
#[test]
fn a_wire_from_0v_to_earth_is_the_bond() {
    for pe_first in [true, false] {
        let (m, pe, zero) = pe_meets(pe_first, false);
        assert!(findings_of(&m, "EW-POT-001").is_empty(), "pe_first {pe_first}");
        let q = ew_derive::Q::new(&m);
        let part = q.get::<ew_derive::PotentialPartition>(());
        let at = |f: u128| {
            part.potential_at(ew_model::EndpointKey::Cp(uid(f).cast(), 1))
                .map(|p| p.id)
                .expect("a potential")
        };
        let (zero_volt, earth) = (at(zero), at(pe));
        assert_ne!(zero_volt, earth, "pe_first {pe_first}: two potentials");
        assert_eq!(
            q.get::<ew_derive::PotentialAttrsOf>(zero_volt)
                .ptype
                .as_ref()
                .map(|t| t.value.clone()),
            Some(ew_derive::ew_schema::PotentialType::M),
            "pe_first {pe_first}: the 0 V keeps its type"
        );
        let bonds = ew_derive::bonds_of_potential(&q, zero_volt);
        assert_eq!(bonds.len(), 1, "pe_first {pe_first}: {bonds:#?}");
        assert!(bonds[0].drawn && bonds[0].via == Some(uid(0x2001).cast()));
        assert_eq!((bonds[0].a, bonds[0].b), (Some(zero_volt), Some(earth)));
        // The one bond is the PELV circuit's one earthing point.
        assert!(findings_of(&m, "EW-POT-002").is_empty(), "pe_first {pe_first}");
    }
    // A live pin wired to earth is no bond: it stays a fault.
    let (m, _, _) = pe_meets(true, true);
    assert_eq!(findings_of(&m, "EW-POT-001").len(), 1);
}

#[test]
fn pot001_joins_several_conflicts_into_one_finding() {
    let m = two_defs(
        &json!({"voltage": "24", "circuit_class": "control"}),
        &json!({"voltage": "230", "circuit_class": "mains"}),
    );
    let f = findings_of(&m, "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message().contains("cannot carry two voltages")
            && f[0].message().contains("two circuit classes"),
        "{}",
        f[0].message()
    );
}

#[test]
fn pot001_agreeing_definitions_have_no_finding() {
    let both =
        json!({"ptype": "l_plus", "voltage": "24", "env": {"ac_dc": "dc"}, "circuit_class": "control"});
    let m = two_defs(&both, &both);
    assert!(findings_of(&m, "EW-POT-001").is_empty());
}

/// The flow conflict names the pass-through function by its device tag, not by an ID, so the text
/// is the same whichever IDs the records carry.
#[test]
fn the_flow_conflict_names_the_pass_through_by_its_tag() {
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
        assert!(f[0].message().contains("K7"), "{}", f[0].message());
        for n in [0x1001, 0x1002, 0x1003] {
            assert!(
                !f[0].message().contains(&us(n)),
                "no ID in the text: {}",
                f[0].message()
            );
        }
        messages.push(f[0].message().clone());
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
    assert_eq!(
        f[0].message(),
        "-1K4099 carries plus L+ through to neutral N: a contact or a fuse passes one potential on, it cannot change \
         it into another. Correct the type on one side, or put a power supply or a transformer between them."
    );
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
                    // Plain words from the Fluent pattern, never the derivation's arm-and-root text:
                    // the potential once, first; then which junction, where, and the wires that meet there (FT-933).
                    let p = &f[0].subject;
                    assert_eq!(
                        f[0].message(),
                        format!(
                            "{p}: it is unclear which wire feeds the junction on folio 1 (wires 1.1); draw one wire \
                             into it from the supply side"
                        ),
                        "case {c}"
                    );
                    assert_eq!(
                        ew_checks::messages::format(&f[0], "en-GB").as_deref(),
                        Some(f[0].message().as_str()),
                        "case {c}: the en-GB pattern renders the message"
                    );
                    let nl = ew_checks::messages::render(&f[0], "nl-NL");
                    assert_eq!(
                        nl,
                        format!(
                            "{p}: het is niet duidelijk welke draad de aftakking op folio 1 voedt (draden 1.1); teken \
                             één draad ernaartoe vanaf de voedingszijde"
                        ),
                        "case {c}"
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
/// to 0x1004), so that the conflicting wire lies in a bond closure of two potentials.
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
    fx.cable(0x6000, 1);
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
fn a_wire_note_in_a_bond_closure_is_reported_once() {
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
    fx.strip(STRIP, 1);
    fx.rail_terminal(0x4001, STRIP, location, pe);
    fx.build()
}

#[test]
fn pot008_location_cases() {
    rule_table(
        "EW-POT-008",
        vec![
            // The warning stays in a project with two or more own locations.
            Case::reports(
                "a rail-bonded terminal without a location is reported",
                rail_model(None, Some(0x1001), true),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005008", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].targets, vec![target(Kind::Device, 0x4001)], "case {c}");
                    assert!(
                        f[0].message().contains("no location"),
                        "case {c}: {}",
                        f[0].message()
                    );
                    assert!(
                        f[0].message().contains("2 own locations"),
                        "case {c}: {}",
                        f[0].message()
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

/// The -X1032:1-4 case: in a project with one own location (an
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
    fx.strip(STRIP, 1);
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

/// A PELV 0 V (an M-role pin with a definition) wired straight to `n` protective-earth pins.
fn drawn_bonds(n: u128) -> Model {
    let mut fx = Fx::base();
    fx.func_m(0x1001, P1);
    fx.def(0x5001, Ep::Cp(0x1001, 1), &pelv_m());
    for i in 0..n {
        fx.func_pe(0x1002 + i, P1);
        fx.conn(0x2001 + i, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1002 + i, 1), &none());
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
                        f[0].message()
                            .contains("its 0 V is not bonded to protective earth"),
                        "case {c}: {}",
                        f[0].message()
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
                        f[0].message().contains("bonded to protective earth in 2 places"),
                        "case {c}: {}",
                        f[0].message()
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
            Case::silent("a pelv 0 V with one wire drawn to earth", drawn_bonds(1)),
            Case::reports(
                "a pelv 0 V with two wires drawn to earth is reported",
                drawn_bonds(2),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].discriminator, "pelv", "case {c}");
                    // A drawn bond is targeted by its wire.
                    for w in [0x2001, 0x2002] {
                        assert!(
                            f[0].targets.contains(&target(Kind::Connection, w)),
                            "case {c}: {:?}",
                            f[0].targets
                        );
                    }
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
// EW-POT-001's quick fix "Declare 0 V–PE bond (PELV)".

const M_FN: u128 = 0x1001;
const TERMINAL: u128 = 0x1002;
const PE_FN: u128 = 0x1003;
const W1: u128 = 0x2001;
const W2: u128 = 0x2002;
const W27: u128 = 0x2027;
const PE_FN2: u128 = 0x1004;
const W30: u128 = 0x2030;

/// 0 V (the M-role CP of `M_FN` on page 1) and PE (the PE-role CPs of `PE_FN` and `PE_FN2` on page 2, joined by wire
/// W30) joined through an interruption-point pair "SUP" and a terminal on page 2: wire W1 from the 0 V CP to the point
/// on page 1, wire W2 from its partner to the terminal's one side, wire W27 from its other side to the PE CP. The 0 V
/// reaches W27 only through the terminal's short, which no page's nets hold, so W27 is no drawn bond: 0 V and PE meet
/// (since FT-1821 a 0 V carried in by the point alone is earthed by W27); `bond` adds the bond the fix declares.
/// `(w1, w27)` are the IDs of W1 and W27.
fn zero_volt_through_a_terminal_with(bond: bool, (w1, w27): (u128, u128)) -> Model {
    let mut fx = Fx::base();
    fx.func_m(M_FN, P1);
    fx.ip(0x1101, P1, "SUP", "source");
    fx.ip(0x1102, P2, "SUP", "target");
    shorted(&mut fx, TERMINAL, P2);
    fx.func_pe(PE_FN, P2);
    fx.func_pe(PE_FN2, P2);
    fx.conn(w1, P1, Ep::Cp(M_FN, 1), cp(0x1101), &none());
    fx.conn(W2, P2, cp(0x1102), cp(TERMINAL), &none());
    fx.conn(w27, P2, Ep::Cp(TERMINAL, 1), Ep::Cp(PE_FN, 1), &none());
    fx.conn(W30, P2, Ep::Cp(PE_FN, 1), Ep::Cp(PE_FN2, 1), &none());
    if bond {
        fx.push(json!({
            "$k": "bond", "id": us(0x4001), "a": {"cp": [us(TERMINAL), 1]}, "b": {"cp": [us(PE_FN), 1]},
            "kind": "pelv_0v_pe", "via": us(w27),
        }));
    }
    fx.build()
}

/// A terminal `n` on `page` whose two sides are one signal (a short): the 0 V passes it at the potential's level, which
/// the page's nets do not hold.
fn shorted(fx: &mut Fx, n: u128, page: u128) {
    const FN_SHORT: u128 = 0x5D0;
    const SYM_SHORT: u128 = 0x5D1;
    fx.fndef_with(
        FN_SHORT,
        SYM_SHORT,
        &[(0, "1", Value::Null), (1, "2", Value::Null)],
        &json!([{"a": 0, "b": 1, "kind": "short"}]),
        &json!({}),
    );
    fx.func_of(n, FN_SHORT, SYM_SHORT, page);
}

fn zero_volt_through_a_terminal(bond: bool) -> Model {
    zero_volt_through_a_terminal_with(bond, (W1, W27))
}

#[test]
fn pot001_offers_the_0v_pe_bond_on_the_wire_at_the_pe_side() {
    // The two ID orders of the wires: the carrier follows the drawing's printed keys, not the IDs.
    for (w1, w27) in [(W1, W27), (W27, W1)] {
        let f = findings_of(&zero_volt_through_a_terminal_with(false, (w1, w27)), "EW-POT-001");
        assert_eq!(f.len(), 1, "{f:#?}");
        assert_eq!(
            f[0].message(),
            "-1K4097:- (0 V) is joined to the protective earth -2K4099:PE on page 2, but not by a wire drawn from \
             the 0 V straight to an earth point. Select the wire that joins them and choose Draw ▸ Connect ▸ Bond, or \
             take that wire away."
        );
        let fix = f[0].fix.as_ref().expect("the bond fix");
        assert_eq!(fix.label, "Declare 0 V–PE bond (PELV)");
        assert_eq!(fix.command, "bond.create");
        // W1 and W27 split 0 V from PE (W30 leaves a PE CP on each side); W27 has an end on the PE-role CP, so it
        // comes first (D3).
        assert_eq!(
            fix.params,
            json!({
                "a": {"cp": {"function": us(TERMINAL), "idx": 1}},
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

/// The 0 V comes in through an interruption point and a terminal's short to the PE CP, with two parallel wires on each
/// side of the point and of the terminal: removing any one splits nothing, so there is no bond carrier.
fn zero_volt_meets_pe_without_a_carrier() -> Model {
    let mut fx = Fx::base();
    fx.func_m(M_FN, P1);
    fx.ip(0x1101, P1, "SUP", "source");
    fx.ip(0x1102, P2, "SUP", "target");
    shorted(&mut fx, TERMINAL, P2);
    fx.func_pe(PE_FN, P2);
    fx.conn(0x2003, P1, Ep::Cp(M_FN, 1), cp(0x1101), &none());
    fx.conn(0x2004, P1, Ep::Cp(M_FN, 1), cp(0x1101), &none());
    fx.conn(W1, P2, cp(0x1102), cp(TERMINAL), &none());
    fx.conn(W27, P2, cp(0x1102), cp(TERMINAL), &none());
    fx.conn(0x2040, P2, Ep::Cp(TERMINAL, 1), Ep::Cp(PE_FN, 1), &none());
    fx.conn(0x2041, P2, Ep::Cp(TERMINAL, 1), Ep::Cp(PE_FN, 1), &none());
    fx.build()
}

#[test]
fn pot001_without_a_carrier_has_no_fix_and_names_draw_connect_bond() {
    let f = findings_of(&zero_volt_meets_pe_without_a_carrier(), "EW-POT-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].fix.is_none(), "{:?}", f[0].fix);
    // The menu path as the menus give it (Draw ▸ Connect ▸ Bond).
    assert_eq!(
        f[0].message(),
        "-1K4097:- (0 V) is joined to the protective earth -2K4099:PE on page 2, but not by a wire drawn from the \
         0 V straight to an earth point. Select the wire that joins them and choose Draw ▸ Connect ▸ Bond, or take \
         that wire away."
    );
    // The Dutch sentence names the Dutch menu path.
    assert_eq!(
        ew_checks::messages::render(&f[0], "nl-NL"),
        "-1K4097:- (0 V) is verbonden met de beschermingsaarde -2K4099:PE op pagina 2, maar niet met een draad \
         die van de 0 V recht naar een aardpunt loopt. Selecteer de draad die ze verbindt en kies Tekenen ▸ \
         Verbinden ▸ Potentiaalverbinding, of haal die draad weg."
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
    assert!(!f[0].message().contains("Draw"), "{}", f[0].message());
}

/// A type-M definition on a terminal on page 1, carried by the interruption points "SUP" to page 2, where the point is
/// wired to one side of a terminal whose other side is wired to `n` PE-role functions: the 0 V reaches those wires only
/// through the terminal's short, so each wire merges, one member-CP conflict each.
fn pe_members_against_m(n: u128) -> ew_checks::Finding {
    let mut fx = Fx::base();
    fx.func(TERMINAL, P1);
    fx.def(0x5001, cp(TERMINAL), &json!({"ptype": "m"}));
    fx.ip(0x1201, P1, "SUP", "source");
    fx.ip(0x1202, P2, "SUP", "target");
    fx.conn(0x2001, P1, cp(TERMINAL), cp(0x1201), &none());
    shorted(&mut fx, 0x1300, P2);
    fx.conn(0x2050, P2, cp(0x1202), cp(0x1300), &none());
    for i in 0..n {
        fx.func_pe(0x1100 + i, P2);
        fx.conn(0x2100 + i, P2, Ep::Cp(0x1300, 1), Ep::Cp(0x1100 + i, 1), &none());
    }
    attrs_finding(&fx.build())
}

/// EW-POT-001's message names at most three member CPs and then counts the rest; nl-NL renders the same cut from the
/// arguments. Three member CPs or fewer are all named, with no count.
#[test]
fn pot001_names_three_member_cps_and_counts_the_rest() {
    let joined = "is joined to the protective earth ";
    let f = pe_members_against_m(3);
    assert_eq!(f.message().matches(joined).count(), 3, "{}", f.message());
    assert!(!f.message().contains("more"), "{}", f.message());
    let f = pe_members_against_m(6);
    assert_eq!(f.message().matches(joined).count(), 3, "{}", f.message());
    assert!(
        f.message()
            .ends_with(" 3 more connection points are wired the same way."),
        "{}",
        f.message()
    );
    let nl = ew_checks::messages::render(&f, "nl-NL");
    assert!(
        nl.ends_with(" Nog 3 aansluitpunten zijn op dezelfde manier bedraad."),
        "{nl}"
    );
    assert_eq!(
        nl.matches("is verbonden met de beschermingsaarde -2K435").count(),
        3,
        "{nl}"
    );
}

/// The 0 V side of [`zero_volt_to_earth`].
#[derive(Clone, Copy, Debug)]
enum ZeroSide {
    /// A pin whose role is M.
    Pin,
    /// A plain point with a definition of type M chosen from the list.
    Defined,
    /// A plain point with a definition named `0V` and no type.
    Named,
    /// An interruption point named `0V` (the 0 V from another page).
    Point,
}

/// The earth side of [`zero_volt_to_earth`].
#[derive(Clone, Copy, Debug)]
enum EarthSide {
    /// The protective earth symbol (a pin whose role is PE).
    Protective,
    /// The functional earth symbol (a pin whose role is FE).
    Functional,
    /// A rail-bonded PE terminal.
    Terminal,
    /// A plain point of the PE rail's potential (a definition of type PE).
    Rail,
    /// An interruption point named `PE`.
    Point,
}

/// A 0 V point (`zero`) wired to an earth point (`earth`) on page 1, the 0 V also wired on to a plain function:
/// returns the model and the endpoints of the two sides.
fn zero_volt_to_earth(zero: ZeroSide, earth: EarthSide) -> (Model, Ep, Ep) {
    const FN_FE: u128 = 0x5C0;
    const SYM_FE: u128 = 0x5C1;
    let mut fx = Fx::base();
    fx.fndef_with(
        FN_FE,
        SYM_FE,
        &[(0, "FE", json!({"potential_role": "fe"}))],
        &json!([]),
        &json!({"category": ["EarthingBar", "FunctionalEarth", "Point"]}),
    );
    let z = match zero {
        ZeroSide::Pin => {
            fx.func_m(0x1001, P1);
            Ep::Cp(0x1001, 1)
        }
        ZeroSide::Defined | ZeroSide::Named => {
            fx.func(0x1001, P1);
            let def = match zero {
                ZeroSide::Defined => json!({"name": "SUP-", "ptype": "m"}),
                _ => json!({"name": "0V"}),
            };
            fx.def(0x5001, cp(0x1001), &def);
            cp(0x1001)
        }
        ZeroSide::Point => {
            fx.ip(0x1001, P1, "0V", "target");
            cp(0x1001)
        }
    };
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, z, cp(0x1002), &none());
    let e = match earth {
        EarthSide::Protective => {
            fx.func_pe(0x1003, P1);
            Ep::Cp(0x1003, 1)
        }
        EarthSide::Functional => {
            fx.func_of(0x1003, FN_FE, SYM_FE, P1);
            cp(0x1003)
        }
        EarthSide::Terminal => {
            fx.strip(STRIP, 1);
            fx.rail_terminal(0x4001, STRIP, None, Some(0x1003));
            Ep::Cp(0x1003, 1)
        }
        EarthSide::Rail => {
            fx.func(0x1003, P1);
            fx.def(0x5003, cp(0x1003), &json!({"name": "PE", "ptype": "pe"}));
            cp(0x1003)
        }
        EarthSide::Point => {
            fx.ip(0x1003, P1, "PE", "target");
            cp(0x1003)
        }
    };
    fx.conn(0x2002, P1, z, e, &none());
    (fx.build(), z, e)
}

/// Every way a drawing earths a 0 V gives no EW-POT-001: the 0 V as a pin's role, a definition of type M, a definition
/// named 0V without a type, or an interruption point named 0V; the earth as the protective or the functional earth
/// symbol, a PE terminal, the PE rail's potential or an interruption point named PE. The wire is the bond (or, to
/// functional earth, the 0 V earthed for function): the two sides stay two potentials.
#[test]
fn every_way_a_0v_is_earthed_is_no_type_conflict() {
    use ZeroSide as Z;
    for zero in [Z::Pin, Z::Defined, Z::Named, Z::Point] {
        for earth in [
            EarthSide::Protective,
            EarthSide::Functional,
            EarthSide::Terminal,
            EarthSide::Rail,
            EarthSide::Point,
        ] {
            let (m, z, e) = zero_volt_to_earth(zero, earth);
            let f = findings_of(&m, "EW-POT-001");
            assert!(f.is_empty(), "{zero:?} to {earth:?}: {f:#?}");
            let q = ew_derive::Q::new(&m);
            let part = q.get::<ew_derive::PotentialPartition>(());
            let at = |e: Ep| {
                let Ep::Cp(n, i) = e else { unreachable!() };
                part.potential_at(ew_model::EndpointKey::Cp(uid(n).cast(), i))
                    .map(|p| p.id)
            };
            assert_ne!(at(z), at(e), "{zero:?} to {earth:?}: two potentials");
        }
    }
}

/// A 0 V wired to the functional earth symbol is earthed for function, not bonded: EW-POT-002 says so in words and
/// names the protective earth symbol and the PE rail as the way.
#[test]
fn a_0v_on_functional_earth_is_told_where_the_bond_belongs() {
    let (m, _, _) = zero_volt_to_earth(ZeroSide::Pin, EarthSide::Functional);
    let f = findings_of(&m, "EW-POT-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message().contains("goes to functional earth (FE) only")
            && f[0]
                .message()
                .contains("protective earth symbol (PE) or to the PE rail"),
        "{}",
        f[0].message()
    );
    let (m, _, _) = zero_volt_to_earth(ZeroSide::Pin, EarthSide::Protective);
    assert!(findings_of(&m, "EW-POT-002").is_empty());
}
