// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! Potential conflicts (doc 08 §3.6): nothing merges silently, the first definition in ID order wins.
//!
//! - `voltage_conflict_between_definitions_is_noted`: 24 V against 230 V is EW-POT-001, 24 V applies.
//! - `env_conflict_between_definitions_is_noted`: DC against AC is EW-POT-001, DC applies.
//! - `class_conflict_between_definitions_is_noted`: control against mains is EW-POT-001, control applies.
//! - `defined_type_contradicting_a_member_role_is_noted`: type `n` against a member role `l_plus`.
//! - `compatible_roles_on_a_defined_type_are_silent`: an FE role on a PE definition is no conflict (doc 08 §3.7).
//! - `agreeing_definitions_are_silent`: equal attributes and a matching role give no EW-POT-001.

use crate::common;

use common::{Fx, id, uid};
use ew_derive::{DeriveNote, PotentialAttrsOf, PotentialId, PotentialNameOf, PotentialPartition, Q};
use ew_model::Model;
use ew_schema::{AcDc, CircuitClass, PotentialEnv, PotentialType};
use ew_units::Dec3;
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const FNDEF_ROLE: u128 = 0x502;
const FNDEF_FE: u128 = 0x503;
const SYM: u128 = 0x510;
const SYM_ROLE: u128 = 0x512;
const SYM_FE: u128 = 0x513;
const P1: u128 = 0x100;

/// One page, a plain two-CP function definition and one whose CP 0 has the role `l_plus`.
fn base() -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.lib_item(
        "fndef",
        FNDEF_ROLE,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Plain"], "contact_kind": "pole",
            "cps": [
                {"idx": 0, "designation": "1", "potential_role": "l_plus"},
                {"idx": 1, "designation": "2"},
            ],
        }),
    );
    fx.symbol(
        SYM_ROLE,
        FNDEF_ROLE,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.lib_item(
        "fndef",
        FNDEF_FE,
        json!({
            "meta": {}, "category": ["Misc", "Misc", "Plain"], "contact_kind": "pole",
            "cps": [
                {"idx": 0, "designation": "1", "potential_role": "fe"},
                {"idx": 1, "designation": "2"},
            ],
        }),
    );
    fx.symbol(
        SYM_FE,
        FNDEF_FE,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)],
    );
    fx.page(P1, 1, None, "multi_line", &[], "one");
    fx
}

/// A plain function `n` (no device) drawn on `page`; with `role` it uses the role definition.
fn func(fx: &mut Fx, n: u128, page: u128, role: bool) {
    let (fndef, sym) = if role {
        (FNDEF_ROLE, SYM_ROLE)
    } else {
        (FNDEF, SYM)
    };
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), fndef, true);
    fx.rep(n + 0x10_000, page, sym, &[n], 0, n as i64 * 1_000_000);
}

/// A wire `n` on `page` between CP 0 of the functions `a` and `b`.
fn wire(fx: &mut Fx, n: u128, page: u128, a: u128, b: u128) {
    fx.push(json!({
        "$k": "connection", "id": uid(n), "page": uid(page),
        "a": {"cp": [uid(a), 0]}, "b": {"cp": [uid(b), 0]},
    }));
}

/// A potential definition `n` on CP 0 of function `at` with the extra members `extra`.
fn def(fx: &mut Fx, n: u128, at: u128, extra: Value) {
    let mut r = json!({"$k": "potential_def", "id": uid(n), "at": {"cp": [uid(at), 0]}, "name": "P"});
    for (k, v) in extra.as_object().expect("object") {
        r[k] = v.clone();
    }
    fx.push(r);
}

/// Two functions on one wire, the lower-ID definition on the first and the higher-ID one on the second.
fn two_defs(first: Value, second: Value, role_on_first: bool) -> Model {
    let mut fx = base();
    func(&mut fx, 0x1001, P1, role_on_first);
    func(&mut fx, 0x1002, P1, false);
    wire(&mut fx, 0x2001, P1, 0x1001, 0x1002);
    // Insert the higher ID first: the natural ID order decides, not the record order.
    def(&mut fx, 0x5002, 0x1002, second);
    def(&mut fx, 0x5001, 0x1001, first);
    fx.build()
}

/// The own attributes of the first potential.
fn attrs_of0(q: &Q, part: &ew_derive::Partition) -> std::sync::Arc<ew_derive::PotentialAttrsV> {
    q.get::<PotentialAttrsOf>(part.potentials().next().unwrap().id)
}

fn pot_notes(q: &mut Q) -> Vec<String> {
    let part = q.get::<PotentialPartition>(());
    // The attribute notes are pushed by the per-potential name family.
    for p in part.potentials() {
        q.get::<PotentialNameOf>(p.id);
    }
    q.diagnostics()
        .notes()
        .iter()
        .filter_map(|n| match n {
            DeriveNote::Connectivity { code, detail, .. } if *code == "EW-POT-001" => Some(detail.clone()),
            _ => None,
        })
        .collect()
}

/// The EW-POT-001 notes of the model, identical for a second `Q`.
fn notes_of(m: &Model) -> Vec<String> {
    let first = pot_notes(&mut Q::new(m));
    assert_eq!(first, pot_notes(&mut Q::new(m)), "deterministic");
    first
}

#[test]
fn voltage_conflict_between_definitions_is_noted() {
    let m = two_defs(json!({"voltage": "24"}), json!({"voltage": "230"}), false);
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potential_count(), 1);
    let p = part.potentials().next().unwrap();
    assert_eq!(p.id, PotentialId::Def(id(0x5001)));
    let attrs = q.get::<PotentialAttrsOf>(p.id);
    assert_eq!(
        attrs.voltage.as_ref().map(|v| v.value),
        Some("24".parse::<Dec3>().unwrap())
    );
    let notes = notes_of(&m);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("voltage") && notes[0].contains("230") && notes[0].contains("24"));
}

#[test]
fn env_conflict_between_definitions_is_noted() {
    let m = two_defs(
        json!({"env": {"ac_dc": "dc"}}),
        json!({"env": {"ac_dc": "ac"}}),
        false,
    );
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let want = PotentialEnv {
        ac_dc: Some(AcDc::Dc),
        ..PotentialEnv::default()
    };
    assert_eq!(
        attrs_of0(&q, &part).env.as_ref().map(|e| e.value.clone()),
        Some(want)
    );
    let notes = notes_of(&m);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("environment"));
}

#[test]
fn class_conflict_between_definitions_is_noted() {
    let m = two_defs(
        json!({"circuit_class": "control"}),
        json!({"circuit_class": "mains"}),
        false,
    );
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(
        attrs_of0(&q, &part)
            .circuit_class
            .as_ref()
            .map(|c| c.value.clone()),
        Some(CircuitClass::Control)
    );
    let notes = notes_of(&m);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("circuit class"));
}

#[test]
fn defined_type_contradicting_a_member_role_is_noted() {
    let m = two_defs(json!({"ptype": "n"}), json!({}), true);
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(
        attrs_of0(&q, &part).ptype.as_ref().map(|t| t.value.clone()),
        Some(PotentialType::N)
    );
    let notes = notes_of(&m);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("LPlus") && notes[0].contains('N'));
}

#[test]
fn agreeing_definitions_are_silent() {
    let both = json!({
        "ptype": "l_plus", "voltage": "24", "env": {"ac_dc": "dc"}, "circuit_class": "control",
    });
    let m = two_defs(both.clone(), both, true);
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(
        attrs_of0(&q, &part).ptype.as_ref().map(|t| t.value.clone()),
        Some(PotentialType::LPlus)
    );
    let notes = notes_of(&m);
    assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn compatible_roles_on_a_defined_type_are_silent() {
    // An FE-role CP on a potential defined as PE is the EW-POT-003 check, not a type conflict.
    let mut fx = base();
    fx.device(0x21_001, "K", Some(1));
    fx.function(0x1001, Some(0x21_001), FNDEF_FE, true);
    fx.rep(0x11_001, P1, SYM_FE, &[0x1001], 0, 1_000_000);
    func(&mut fx, 0x1002, P1, false);
    wire(&mut fx, 0x2001, P1, 0x1001, 0x1002);
    def(&mut fx, 0x5001, 0x1002, json!({"ptype": "pe"}));
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    assert_eq!(part.potential_count(), 1);
    assert_eq!(
        attrs_of0(&q, &part).ptype.as_ref().map(|t| t.value.clone()),
        Some(PotentialType::Pe)
    );
    let notes = notes_of(&m);
    assert!(notes.is_empty(), "{notes:?}");
}
