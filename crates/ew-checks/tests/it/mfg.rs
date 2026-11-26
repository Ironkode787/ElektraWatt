// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules of a wire's ends (doc 13 §4.6) over a small drawing: a black box whose part gives its pin 1 a clamp, and
//! two wires that land on it. EW-MFG-008: an insulation-displacement clamp cannot take the fine-stranded wires (no
//! wire part names a class: fine-stranded is assumed); EW-POT-010: two conductors land on a clamp that takes one.

use crate::common;

use common::*;
use ew_model::Model;
use ew_schema::Kind;
use serde_json::{Value, json};

const PART: u128 = 0x7001;
const BOX: u128 = 0x1001;
const BOX_DEVICE: u128 = 0x3001;

/// The black box with its clamp `clamp` on pin 1, and `wires` wires from it to functions of their own.
fn clamp(clamp: &Value, wires: u128) -> Model {
    clamp_with(clamp, wires, &none())
}

/// [`clamp`] with the wires' members `wire` (their overrides).
fn clamp_with(clamp: &Value, wires: u128, wire: &Value) -> Model {
    let mut fx = Fx::base();
    let mut pin = json!({"idx": 0, "designation": "1"});
    for (k, v) in clamp.as_object().expect("an object") {
        pin[k] = v.clone();
    }
    fx.part(
        PART,
        "BOX-1",
        &json!({"cp_pattern": [pin, {"idx": 1, "designation": "2"}]}),
    );
    fx.device(BOX_DEVICE, "A", Some(1))
        .with_parts(BOX_DEVICE, &[PART]);
    fx.drawn_function(BOX, BOX_DEVICE, P1, [0, 0]);
    fx.rec_mut(&us(BOX))["kind"] = json!("black_box");
    for n in 0..wires {
        fx.func(0x1100 + n, P1);
        fx.conn(0x2001 + n, P1, cp(BOX), cp(0x1100 + n), wire);
    }
    fx.build()
}

#[test]
fn mfg008_reports_a_fine_stranded_wire_into_an_idc_clamp() {
    rule_table(
        "EW-MFG-008",
        vec![
            Case::reports("idc", clamp(&json!({"clamp_type": "idc"}), 2), |c, f| {
                assert_eq!(f.len(), 2, "case {c}: {f:#?}");
                assert_eq!(f[0].code, "140008");
                assert_eq!(f[0].severity, ew_checks::Severity::Warning);
                assert!(f.iter().all(|x| x.targets[0].kind == Kind::Connection), "{f:#?}");
                assert!(
                    f[0].message()
                        .contains("fine-stranded wire into an insulation-displacement clamp"),
                    "{}",
                    f[0].message()
                );
                assert_worded(f);
            }),
            Case::silent("screw", clamp(&json!({"clamp_type": "screw"}), 2)),
            Case::silent("no clamp data", clamp(&json!({}), 2)),
        ],
    );
}

#[test]
fn pot010_reports_more_conductors_than_the_clamp_takes() {
    rule_table(
        "EW-POT-010",
        vec![
            Case::reports(
                "two on one",
                clamp(&json!({"clamp_type": "screw", "max_conductors": 1}), 2),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "005010");
                    assert_eq!(f[0].severity, ew_checks::Severity::Error);
                    assert_eq!(f[0].targets, vec![target(Kind::Function, BOX)]);
                    assert!(
                        f[0].message().ends_with("takes 1 conductor; 2 land on it"),
                        "{}",
                        f[0].message()
                    );
                    assert_worded(f);
                },
            ),
            Case::silent(
                "one on one",
                clamp(&json!({"clamp_type": "screw", "max_conductors": 1}), 1),
            ),
            Case::silent("capacity not named", clamp(&json!({"clamp_type": "screw"}), 3)),
            Case::reports(
                "a size outside the range",
                clamp_with(
                    &json!({"clamp_type": "screw", "csa_min": "0.5", "csa_max": "1.5"}),
                    1,
                    &json!({"overrides": {"csa": "2.5"}}),
                ),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert!(
                        f[0].message().contains("takes 0.5–1.5 mm²; ")
                            && f[0].message().ends_with("of 2.5 mm² lands on it"),
                        "{}",
                        f[0].message()
                    );
                    assert_worded(f);
                },
            ),
            Case::silent(
                "a size inside the range",
                clamp_with(
                    &json!({"clamp_type": "screw", "csa_min": "0.5", "csa_max": "2.5"}),
                    1,
                    &json!({"overrides": {"csa": "2.5"}}),
                ),
            ),
        ],
    );
}

/// One wire from -K<counter> to -K2 whose end at the first is set bare by hand (`end_a`: the end the connection reaches
/// through its endpoint a). The wire reads From the lower tag: numbering the first 3 turns the wire round, and the end
/// set by hand stays at its connection point.
#[test]
fn an_end_set_by_hand_stays_with_its_cp_when_the_wire_turns_round() {
    let mut from = Vec::new();
    for counter in [1, 3] {
        let mut fx = Fx::base();
        fx.func_tagged(0x1001, counter, P1);
        fx.func_tagged(0x1002, 2, P1);
        fx.conn(
            0x2001,
            P1,
            cp(0x1001),
            cp(0x1002),
            &json!({"overrides": {"end_a": {"treatment": "bare"}}}),
        );
        let m = fx.build();
        let q = ew_derive::Q::new(&m);
        let set = q.get::<ew_derive::WireRecordSet>(());
        let r = set.first().expect("the wire");
        let at = (r.wire.ends.iter())
            .position(|e| e.cp.function.to_string() == us(0x1001))
            .expect("its end at the first function");
        assert_eq!(
            (r.ends[at].treatment, r.ends[at].treated_by),
            (ew_derive::EndTreatment::Bare, ew_derive::TreatedBy::Hand),
            "counter {counter}"
        );
        assert!(
            r.ends[1 - at].treated_by != ew_derive::TreatedBy::Hand,
            "counter {counter}"
        );
        from.push(at);
    }
    assert_eq!(from, [0, 1], "the wire turned round");
}

/// EW-MFG-009's finding about an end names the clamp and the wire, not the wire's From or To: a twin ferrule set by hand
/// that no second wire shares keeps its fingerprint when numbering the first device 3 turns the wire round.
#[test]
fn an_end_s_finding_keeps_its_fingerprint_when_the_wire_turns_round() {
    let fingerprints: Vec<_> = [1, 3]
        .into_iter()
        .map(|counter| {
            let mut fx = Fx::base();
            fx.func_tagged(0x1001, counter, P1);
            fx.func_tagged(0x1002, 2, P1);
            fx.conn(
                0x2001,
                P1,
                cp(0x1001),
                cp(0x1002),
                &json!({"overrides": {"end_a": {"treatment": "twin_ferrule"}}}),
            );
            let f = findings_of(&fx.build(), "EW-MFG-009");
            assert_eq!(f.len(), 1, "counter {counter}: {f:#?}");
            f[0].fingerprint
        })
        .collect();
    assert_eq!(fingerprints[0], fingerprints[1]);
}

/// EW-POT-010's finding about a wire of a size outside its clamp's range names the wire by its identity: a wire to -K1,
/// which takes the number of the wire to -K9, leaves the finding's fingerprint as it was.
#[test]
fn a_size_finding_keeps_its_fingerprint_when_the_wire_is_renumbered() {
    let model = |thin: bool| {
        let mut fx = Fx::base();
        fx.part(
            PART,
            "BOX-1",
            &json!({"cp_pattern": [
                {"idx": 0, "designation": "1", "clamp_type": "screw", "csa_min": "0.5", "csa_max": "1.5"},
                {"idx": 1, "designation": "2"},
            ]}),
        );
        fx.device(BOX_DEVICE, "A", Some(1))
            .with_parts(BOX_DEVICE, &[PART]);
        fx.drawn_function(BOX, BOX_DEVICE, P1, [0, 0]);
        fx.rec_mut(&us(BOX))["kind"] = json!("black_box");
        fx.func_tagged(0x1100, 9, P1);
        fx.conn(
            0x2001,
            P1,
            cp(BOX),
            cp(0x1100),
            &json!({"overrides": {"csa": "2.5"}}),
        );
        if thin {
            // A wire of a size the clamp takes, to -K1: it numbers first.
            fx.func_tagged(0x1101, 1, P1);
            fx.conn(
                0x2002,
                P1,
                cp(BOX),
                cp(0x1101),
                &json!({"overrides": {"csa": "1"}}),
            );
        }
        let f = findings_of(&fx.build(), "EW-POT-010");
        assert_eq!(f.len(), 1, "{f:#?}");
        (f[0].message(), f[0].fingerprint)
    };
    let (alone, both) = (model(false), model(true));
    assert_ne!(alone.0, both.0, "the wire took another number");
    assert_eq!(alone.1, both.1);
}
