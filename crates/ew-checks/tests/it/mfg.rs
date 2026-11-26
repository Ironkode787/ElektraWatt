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
        fx.conn(0x2001 + n, P1, cp(BOX), cp(0x1100 + n), &none());
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
        ],
    );
}
