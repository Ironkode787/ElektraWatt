// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An external point whose reference holds only what is known of the far end (FT-1876, doc 06 §3.5): a location and
//! a device, an address alone, or nothing. It prints what its reference holds, and EW-CORE-034 still asks for the
//! cable, naming the connection by what is known.

use crate::common;

use common::*;
use ew_derive::ew_schema::Kind;
use ew_model::Model;
use serde_json::{Value, json};

/// An external interruption point `n` named `name` whose reference holds `reference` (members of the record), wired
/// to CP 0 of the function `dev` by connection `conn`, not as a cable's core.
fn point(fx: &mut Fx, n: u128, name: &str, dev: u128, conn: u128, reference: Value) {
    fx.ip(n, P1, name, "target");
    let ext = n + 0x100;
    fx.rec_mut(&us(n))["kind"]["ip"]["external"] = json!(us(ext));
    let mut record = json!({"$k": "external_ref", "id": us(ext)});
    record
        .as_object_mut()
        .expect("an object")
        .extend(reference.as_object().expect("members").clone());
    fx.push(record);
    fx.conn(conn, P1, cp(n), cp(dev), &json!({}));
}

/// Three marks at the head of three current paths: one from `+J-E-206 / N1`, one from the address `Q4.1` alone, one
/// whose far end is not known.
fn marks() -> Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(0x900), "aspect": "location", "code": "J-E-206"}));
    fx.func_tagged(0x1001, 1, P1);
    fx.func_tagged(0x1002, 2, P1);
    fx.func_tagged(0x1003, 3, P1);
    point(
        &mut fx,
        0x3001,
        "Valve 1 open",
        0x1001,
        0x2001,
        json!({"location": us(0x900), "device": "N1"}),
    );
    point(
        &mut fx,
        0x3002,
        "Valve 2 open",
        0x1002,
        0x2002,
        json!({"device": "Q4.1"}),
    );
    point(&mut fx, 0x3003, "Valve 3 open", 0x1003, 0x2003, json!({}));
    fx.build()
}

/// The cross-reference text a mark `n` prints under it.
fn printed(m: &Model, n: u128) -> String {
    let q = ew_derive::Q::new(m);
    q.get::<ew_derive::XrefTextOf>(uid(n + 0x10_000).cast())
        .text
        .clone()
}

#[test]
fn a_mark_prints_what_its_reference_holds() {
    let m = marks();
    assert_eq!(printed(&m, 0x3001), "+J-E-206 / N1");
    assert_eq!(printed(&m, 0x3002), "Q4.1", "an address alone");
    assert_eq!(printed(&m, 0x3003), "", "a far end not known prints nothing");
    // An external point pairs with nothing, whatever its reference holds.
    assert!(findings_of(&m, "EW-CORE-051").is_empty());
}

/// The wires of the three marks are no cable's cores: one finding per place, the location's, the address's, and one
/// that says the far end is not known, in both languages.
#[test]
fn core034_still_asks_for_the_cable_and_names_what_is_known() {
    let f = findings_of(&marks(), "EW-CORE-034");
    let subjects: Vec<&str> = f.iter().map(|f| f.subject.as_str()).collect();
    assert!(subjects.contains(&"+J-E-206 → -1K1"), "{subjects:?}");
    assert!(subjects.contains(&"Q4.1 → -1K2"), "{subjects:?}");
    let unknown = f
        .iter()
        .find(|f| f.subject == "-1K3")
        .unwrap_or_else(|| panic!("{subjects:?}"));
    assert!(
        unknown
            .message
            .starts_with("the external connection from a place not known to -1K3 has no cable: wire "),
        "{}",
        unknown.message
    );
    assert_eq!(ew_checks::messages::render(unknown, "en-GB"), unknown.message);
    let nl = ew_checks::messages::render(unknown, "nl-NL");
    assert!(
        nl.starts_with("de externe verbinding van een onbekende plek naar -1K3 heeft geen kabel"),
        "{nl}"
    );
    assert!(
        unknown
            .targets
            .iter()
            .any(|t| t.kind == Kind::ExternalRef && t.id == uid(0x3103)),
        "the reference is the place: {:?}",
        unknown.targets
    );
}
