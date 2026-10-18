// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Connection-point instantiation (`function_cps`, doc 03 §7, FR-DEV-008): FnDef CPs, overrides, black
//! boxes from a part pattern.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{Cp, FunctionCps, Q};
use ew_schema::PotentialType;
use serde_json::{Value, json};

const FNDEF_MCB: u128 = 0x500;
const FNDEF_BOX: u128 = 0x501;
const PART_PLAIN: u128 = 0x600;
const PART_PATTERN: u128 = 0x601;

fn part_json(number: &str, extra: Value) -> Value {
    let mut p = json!({
        "meta": {},
        "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
        "part_number": number,
    });
    for (k, v) in extra.as_object().expect("object") {
        p[k] = v.clone();
    }
    p
}

fn base() -> Fx {
    let mut fx = Fx::new();
    // MCB 1P+N: duplicate designations "N" at idx 2 and 3 (REF-029).
    fx.fndef(
        FNDEF_MCB,
        ["ProtectionOvercurrent", "MCB", "1P+N"],
        &[(0, "1"), (1, "2"), (2, "N"), (3, "N")],
    );
    fx.fndef(FNDEF_BOX, ["Misc", "Misc", "BlackBox"], &[(0, "X")]);
    fx.lib_item("part", PART_PLAIN, part_json("PLAIN", json!({})));
    fx.lib_item(
        "part",
        PART_PATTERN,
        part_json(
            "PATTERN",
            json!({"cp_pattern": [
                {"idx": 0, "designation": "L", "level": 1, "side": "side1"},
                {"idx": 1, "designation": "+", "potential_role": "l_plus", "side": "side2"},
                {"idx": 2, "designation": "PE", "potential_role": "pe"},
            ]}),
        ),
    );
    fx
}

fn cps(fx: &Fx, f: u128) -> Vec<Cp> {
    let m = fx.build();
    let q = Q::new(&m);
    q.get::<FunctionCps>(id(f)).as_ref().clone()
}

fn designations(cps: &[Cp]) -> Vec<(u16, &str)> {
    cps.iter().map(|c| (c.idx, c.designation.as_str())).collect()
}

#[test]
fn fndef_cps_with_duplicate_designations() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true);
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "1"), (1, "2"), (2, "N"), (3, "N")]);
    for c in &out {
        assert_eq!(c.alias, None);
        assert_eq!(c.description, None);
        assert!(!c.unused);
        assert!(!c.overridden);
        assert!(c.physical);
    }
}

#[test]
fn override_replaces_designation_and_alias() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with(
            "cp_overrides",
            json!([{"idx": 0, "designation": "L1", "alias": "1 / L1", "description": {"en": "Phase"}}]),
        );
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "L1"), (1, "2"), (2, "N"), (3, "N")]);
    assert_eq!(out[0].alias.as_deref(), Some("1 / L1"));
    assert!(out[0].description.is_some());
    assert!(out[0].overridden);
    assert!(out[1..].iter().all(|c| !c.overridden));
}

#[test]
fn override_without_designation_keeps_it_and_sets_unused() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with("cp_overrides", json!([{"idx": 3, "unused": true}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(out[3].designation, "N");
    assert!(out[3].unused);
    assert!(out[3].overridden);
    assert!(!out[2].unused);
}

#[test]
fn override_of_a_missing_idx_is_ignored() {
    let mut fx = base();
    fx.device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), FNDEF_MCB, true)
        .with("cp_overrides", json!([{"idx": 9, "designation": "Z"}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(out.len(), 4);
    assert!(out.iter().all(|c| !c.overridden));
}

#[test]
fn black_box_takes_the_cps_of_the_first_part_with_a_pattern() {
    let mut fx = base();
    fx.device(0x1000, "U", Some(1))
        .with(
            "parts",
            json!([
                {"part": {"copy": uid(PART_PLAIN)}, "qty": "1"},
                {"part": {"copy": uid(PART_PATTERN)}, "qty": "1"},
            ]),
        )
        .function(0x1001, Some(0x1000), FNDEF_BOX, true)
        .with("kind", json!("black_box"))
        .with("cp_overrides", json!([{"idx": 2, "alias": "earth"}]));
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "L"), (1, "+"), (2, "PE")]);
    assert_eq!(out[0].level, Some(1));
    assert!(out[0].side.is_some());
    assert_eq!(out[1].potential_role, Some(PotentialType::LPlus));
    assert!(out.iter().all(|c| c.physical && !c.source_output && !c.optional));
    assert_eq!(out[2].alias.as_deref(), Some("earth"));
    assert!(out[2].overridden);
}

#[test]
fn black_box_without_a_pattern_falls_back_to_the_fndef() {
    let mut fx = base();
    fx.device(0x1000, "U", Some(1))
        .with("parts", json!([{"part": {"copy": uid(PART_PLAIN)}, "qty": "1"}]))
        .function(0x1001, Some(0x1000), FNDEF_BOX, true)
        .with("kind", json!("black_box"));
    let out = cps(&fx, 0x1001);
    assert_eq!(designations(&out), [(0, "X")]);
}

#[test]
fn missing_function_and_missing_def_have_no_cps() {
    // A function naming a missing FnDef copy cannot exist in a valid snapshot (EW-INV-001), so only the
    // missing function is testable here; the missing-def branch is defensive.
    let fx = base();
    assert!(cps(&fx, 0x4242).is_empty());
}
