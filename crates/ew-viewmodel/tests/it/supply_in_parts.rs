// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comes in here… with a field left empty (FT-1876, doc 06 §3.5): an empty field is not known, so a mark named for its
//! signal prints its address alone under it, and one whose far end is not known prints nothing.

use crate::common;

use common::{Tmp, actual_size, done, select_ids, tables};
use serde_json::json;

/// Marks the first interruption point of folio 102 that is no external point, with `location` and `device` as the
/// form gives them; returns what the point prints under it.
fn mark(location: &str, device: &str) -> String {
    let tmp = Tmp::new("supply-in-parts");
    let mut s = actual_size(&tmp, "102");
    let point = tables(&s)
        .reps
        .iter()
        .find(|r| r.ip.as_ref().is_some_and(|ip| !ip.external))
        .expect("an interruption point on 102")
        .id
        .to_string();
    select_ids(&mut s, std::slice::from_ref(&point));
    done(
        &mut s,
        "ip.supply_in",
        json!({"location": location, "device": device}),
    );
    let rep = tables(&s)
        .reps
        .iter()
        .find(|r| r.id.to_string() == point)
        .cloned()
        .expect("the point");
    let ip = rep.ip.expect("still an interruption point");
    assert!(ip.external, "it is an external point");
    ip.continuation
}

#[test]
fn an_empty_field_of_comes_in_here_is_not_known() {
    assert_eq!(mark("", "Q4.1"), "Q4.1", "an address alone");
    assert_eq!(mark("", ""), "", "a far end not known prints nothing");
}
