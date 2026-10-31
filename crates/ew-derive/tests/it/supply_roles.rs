// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 5, PX-2 (D13, FR-PLC-024): the supply groups of a card's PLC part give its pins the potential roles
//! `L+` and `M` when neither the part pattern nor the function definition gives them one.
//!
//! The orchestrator's ruling 2 of cycle 42, as amended after the Opus read: "A CP's role is: the authored CP role, else
//! the definition CP's role (the existing fallback ...), else the supply group's role. The group is read from the part
//! copy the derivation already depends on, through the same memo input as the pins: no new memo key."
//!
//! - `a_black_box_card_takes_l_plus_and_m_from_its_supply_group`: pins "1" and "2" without a role get `L+` and `M`.
//! - `an_authored_role_wins_over_the_supply_group`: a pattern role and an FnDef role are kept.
//! - `the_group_of_the_named_part_use_applies`: `plc_module.part_use` names the part whose groups apply.
//! - `a_group_role_reads_as_an_authored_role_in_names_and_potentials`: the potential on pin "1" has the same type,
//!   source and name whether the role is authored or comes from a group.

use crate::common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{AttrSource, Cp, FunctionCps, PotentialAttrsOf, PotentialNameOf, PotentialPartition, Q};
use ew_schema::PotentialType;
use serde_json::{Value, json};

const FNDEF_BOX: u128 = 0x501;
const FNDEF_LAMP: u128 = 0x502;
const SYM_LAMP: u128 = 0x510;
const PART_CARD: u128 = 0x601;
const PART_ROLES: u128 = 0x602;
const PART_PLAIN: u128 = 0x603;
const P1: u128 = 0x100;
const CARD: u128 = 0x1000;
const CARD_FN: u128 = 0x1001;
const LAMP: u128 = 0x1100;
const LAMP_FN: u128 = 0x1101;

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

/// The pattern of a black-box card: pins "1", "2" (no role unless `roles`) and "3" (role `fe`).
fn pattern(roles: bool) -> Value {
    let role = |r: &str| if roles { json!(r) } else { Value::Null };
    let mut cps = vec![
        json!({"idx": 0, "designation": "1", "potential_role": role("l_plus")}),
        json!({"idx": 1, "designation": "2", "potential_role": role("m")}),
        json!({"idx": 2, "designation": "3", "potential_role": "fe"}),
    ];
    for c in &mut cps {
        if c["potential_role"].is_null() {
            c.as_object_mut().expect("object").remove("potential_role");
        }
    }
    json!(cps)
}

/// The card part's PLC section: a module with the supply group of PX-2's fixture (`l_plus: "1"`, `m: "2"`) and a
/// group naming the earth pin "3" as its M, which an authored role must win over.
fn groups() -> Value {
    json!({
        "role": "module",
        "supply_groups": [
            {"kind": "electronics", "cps": {"l_plus": "1", "m": "2"}},
            {"kind": "load", "cps": {"l_plus": "9", "m": "3"}, "feeds": "outputs"},
        ]
    })
}

/// A card `CARD` with one black-box function on the part `part`, and a lamp wired from its pin 0 to card pin "1".
fn fixture(part: u128) -> Fx {
    let mut fx = Fx::new();
    fx.fndef(FNDEF_BOX, ["Misc", "Misc", "BlackBox"], &[]);
    fx.fndef(FNDEF_LAMP, ["Misc", "Misc", "Lamp"], &[(0, "X1"), (1, "X2")]);
    fx.symbol(
        SYM_LAMP,
        FNDEF_LAMP,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 4_000_000)],
    );
    fx.lib_item(
        "part",
        PART_CARD,
        part_json(
            "CARD-G",
            json!({"cp_pattern": pattern(false), "sections": {"plc": groups()}}),
        ),
    );
    fx.lib_item(
        "part",
        PART_ROLES,
        part_json("CARD-R", json!({"cp_pattern": pattern(true)})),
    );
    fx.lib_item("part", PART_PLAIN, part_json("PLAIN", json!({})));
    fx.page(P1, 1, None, "multi_line", &[], "one");
    fx.device(CARD, "A", Some(1))
        .with("parts", json!([{"part": {"copy": uid(part)}, "qty": "1"}]))
        .function(CARD_FN, Some(CARD), FNDEF_BOX, true)
        .with("kind", json!("black_box"));
    fx.device(LAMP, "E", Some(1))
        .function(LAMP_FN, Some(LAMP), FNDEF_LAMP, true);
    fx.rep(0x2101, P1, SYM_LAMP, &[LAMP_FN], 0, 0);
    fx.push(json!({
        "$k": "connection", "id": uid(0x3001), "page": uid(P1),
        "a": {"cp": [uid(LAMP_FN), 0]}, "b": {"cp": [uid(CARD_FN), 0]},
    }));
    fx
}

fn cps(fx: &Fx) -> Vec<Cp> {
    let m = fx.build();
    let q = Q::new(&m);
    q.get::<FunctionCps>(id(CARD_FN)).as_ref().clone()
}

fn roles(cps: &[Cp]) -> Vec<(&str, Option<PotentialType>)> {
    cps.iter()
        .map(|c| (c.designation.as_str(), c.potential_role.clone()))
        .collect()
}

#[test]
fn a_black_box_card_takes_l_plus_and_m_from_its_supply_group() {
    let out = cps(&fixture(PART_CARD));
    assert_eq!(
        roles(&out),
        [
            ("1", Some(PotentialType::LPlus)),
            ("2", Some(PotentialType::M)),
            // the authored `fe` wins over the second group's `m: "3"`
            ("3", Some(PotentialType::Fe)),
        ]
    );
}

#[test]
fn an_authored_role_wins_over_the_supply_group() {
    // the pattern's roles are authored on the part without groups: the same roles, no group read
    let out = cps(&fixture(PART_ROLES));
    assert_eq!(
        roles(&out),
        [
            ("1", Some(PotentialType::LPlus)),
            ("2", Some(PotentialType::M)),
            ("3", Some(PotentialType::Fe)),
        ]
    );
    // an FnDef role is kept too: a non-black-box function whose FnDef CP "1" has role `pe`, on the grouped card
    let mut fx = fixture(PART_CARD);
    fx.lib_item(
        "fndef",
        0x505,
        json!({"meta": {}, "category": ["Misc", "Misc", "Supply"], "contact_kind": "pole",
               "cps": [{"idx": 0, "designation": "1", "potential_role": "pe"}, {"idx": 1, "designation": "2"}]}),
    );
    fx.function(0x1002, Some(CARD), 0x505, false);
    let m = fx.build();
    let q = Q::new(&m);
    let out = q.get::<FunctionCps>(id(0x1002)).as_ref().clone();
    assert_eq!(
        roles(&out),
        [("1", Some(PotentialType::Pe)), ("2", Some(PotentialType::M))]
    );
}

#[test]
fn the_group_of_the_named_part_use_applies() {
    // part use 0 is the plain part, part use 1 the grouped card: without `part_use` the first use with groups applies
    let mut fx = fixture(PART_CARD);
    fx.record(CARD)["parts"] = json!([
        {"part": {"copy": uid(PART_PLAIN)}, "qty": "1"},
        {"part": {"copy": uid(PART_CARD)}, "qty": "1"},
    ]);
    // the black box takes the first pattern (the plain part has none, so the card's)
    let out = cps(&fx);
    assert_eq!(out[0].potential_role, Some(PotentialType::LPlus));
    // `plc_module.part_use` naming the plain part (no groups): no group role
    fx.record(CARD)["plc_module"] = json!({"part_use": 0});
    let out = cps(&fx);
    assert_eq!(
        roles(&out),
        [("1", None), ("2", None), ("3", Some(PotentialType::Fe))]
    );
}

/// The potential of the wire lamp X1 – card pin "1", with its type, the type's source and the name.
fn potential_of_pin_1(fx: &Fx) -> (Option<PotentialType>, Option<AttrSource>, String) {
    let m = fx.build();
    let q = Q::new(&m);
    let part = q.get::<PotentialPartition>(());
    let p = part
        .potentials()
        .find(|p| p.cps.iter().any(|c| c.function == id(CARD_FN) && c.idx == 0))
        .expect("the potential on card pin 1");
    let attrs = q.get::<PotentialAttrsOf>(p.id);
    let name = q.get::<PotentialNameOf>(p.id);
    (
        attrs.ptype.as_ref().map(|a| a.value.clone()),
        attrs.ptype.as_ref().map(|a| a.source),
        name.name.text.clone(),
    )
}

#[test]
fn a_group_role_reads_as_an_authored_role_in_names_and_potentials() {
    let grouped = potential_of_pin_1(&fixture(PART_CARD));
    let authored = potential_of_pin_1(&fixture(PART_ROLES));
    assert_eq!(grouped, authored);
    assert_eq!(grouped.0, Some(PotentialType::LPlus));
    let source = grouped.1.expect("a source");
    assert!(matches!(source, AttrSource::Role(_)), "{source:?}");
}
