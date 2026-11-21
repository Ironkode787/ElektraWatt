// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! External points whose far end is known in part or not at all (FT-1876, doc 06 §3.5): `ip.supply_in` and
//! `external_ref.create` take a device or an address alone, a location alone, or nothing.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::{CreateExternalRef, MarkSupplyIn};
use ew_ids::Id;
use ew_model::fixture::*;
use ew_schema::{ExternalRef, Function, FunctionKind};

/// Marks `points` as coming from `location` (a code) and `device`, each given or not.
fn mark(points: Vec<Id<Function>>, location: Option<&str>, device: Option<&str>) -> MarkSupplyIn {
    MarkSupplyIn {
        points,
        location: location.map(ident),
        device: device.map(ident),
        remove: false,
        group: None,
    }
}

/// The external reference of interruption point `f`.
fn external_of(e: &ew_commands::Engine, f: Id<Function>) -> Option<Id<ExternalRef>> {
    match &row(e, f).kind {
        FunctionKind::Ip(ip) => ip.external,
        _ => None,
    }
}

/// A mark whose signal comes from an address alone gets a reference of that address and no location; one whose far
/// end is not known a reference that holds nothing, which a second such mark shares.
#[test]
fn ip_supply_in_takes_an_address_alone_or_a_far_end_not_known() {
    let mut e = engine(mini_model());
    let r = e
        .execute(&mark(vec![FN_IP_101], None, Some("Q4.1")))
        .expect("an address alone");
    let x = row(&e, primary::<ExternalRef>(&r));
    assert_eq!((x.location, x.device.clone()), (None, Some(ident("Q4.1"))));
    assert_eq!(external_of(&e, FN_IP_101), Some(x.id));

    let r = e
        .execute(&mark(vec![FN_IP_102], None, None))
        .expect("a far end not known");
    let unknown = row(&e, primary::<ExternalRef>(&r));
    assert_eq!((unknown.location, unknown.device.clone()), (None, None));
    let again = e.execute(&mark(vec![FN_IP_101], None, None)).expect("again");
    assert_eq!(
        primary::<ExternalRef>(&again),
        unknown.id,
        "one reference for a far end not known"
    );
}

/// A location written with its sign (`+UP`) is the node `UP`, the one a mark without the sign finds (FT-1901); the
/// sign alone is refused.
#[test]
fn ip_supply_in_takes_a_location_with_or_without_its_sign() {
    let mut e = engine(mini_model());
    let r = e
        .execute(&mark(vec![FN_IP_101], Some("+UP"), None))
        .expect("a location with its sign");
    let node = row(&e, primary::<ExternalRef>(&r))
        .location
        .expect("a location node");
    assert_eq!(row(&e, node).code, ident("UP"));
    let r = e
        .execute(&mark(vec![FN_IP_102], Some("UP"), None))
        .expect("the same location without its sign");
    assert_eq!(row(&e, primary::<ExternalRef>(&r)).location, Some(node));
    assert!(e.execute(&mark(vec![FN_IP_101], Some("+"), None)).is_err());
}

/// `external_ref.create` takes a location alone, or nothing at all.
#[test]
fn external_ref_create_takes_what_is_known() {
    let mut e = engine(mini_model());
    let at = |location| CreateExternalRef {
        location,
        device: None,
        connection: None,
        project: None,
    };
    let r = e.execute(&at(Some(LOC))).expect("a location alone");
    assert_eq!(row(&e, primary::<ExternalRef>(&r)).location, Some(LOC));
    let r = e.execute(&at(None)).expect("nothing");
    let x = row(&e, primary::<ExternalRef>(&r));
    assert_eq!((x.location, x.device.clone()), (None, None));
}
