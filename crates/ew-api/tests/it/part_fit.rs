// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What fits -102F1 of the sample (doc 07 §4.3 "What fits"): its own breaker fits; a breaker whose clamps take too small
//! a wire, one without the pins it has wired, and one of another kind do not, each with its sentence.

use crate::common;

use ew_derive::{Misfit, part_fit};
use ew_ids::Id;
use ew_schema::{CpData, Device, KeyedList, Part};

/// The part's CP pattern with `edit` applied to each CP.
fn pattern(part: &Part, edit: impl Fn(&mut CpData)) -> KeyedList<CpData> {
    let cps: Vec<CpData> = part
        .cp_pattern
        .iter()
        .cloned()
        .map(|mut cp| {
            edit(&mut cp);
            cp
        })
        .collect();
    KeyedList::from_vec(cps).unwrap()
}

#[test]
fn the_reasons_a_part_does_not_fit_102f1() {
    let s = common::sample();
    let device = s.resolve_reference("tag:-102F1").expect("-102F1");
    let q = s.query();
    let id: Id<Device> = Id::from_uuid(device);
    let own: Part = q
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == "EW-GEN-MCB-1")
        .map(|(_, c)| c.item.clone())
        .expect("the breaker");
    assert_eq!(part_fit(&q, id, &own, true), None, "its own breaker fits");
    assert_eq!(part_fit(&q, id, &own, false), Some(Misfit::Function));
    // Clamps that take 0.1 to 0.2 mm² only: a wire on the first wired pin is larger.
    let mut small = own.clone();
    small.cp_pattern = pattern(&own, |cp| {
        cp.csa_min = Some("0.1".parse().unwrap());
        cp.csa_max = Some("0.2".parse().unwrap());
    });
    let Some(m @ Misfit::ClampTooSmall { .. }) = part_fit(&q, id, &small, true) else {
        panic!("a clamp too small")
    };
    let text = s.misfit_text(device, &m).to_string();
    assert!(
        text.starts_with("Its clamp ") && text.contains(" takes 0.1 to 0.2 mm²; wire "),
        "{text}"
    );
    // No pin of the pattern is wired at the device's pins.
    let mut renamed = own.clone();
    renamed.cp_pattern = pattern(&own, |cp| {
        cp.designation = format!("X{}", cp.designation.as_str()).parse().unwrap();
    });
    let Some(m @ Misfit::MissingPin { .. }) = part_fit(&q, id, &renamed, true) else {
        panic!("a pin it lacks")
    };
    let text = s.misfit_text(device, &m).to_string();
    assert!(
        text.starts_with("It has no pin ") && text.contains("; -102F1:"),
        "{text}"
    );
}
