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

/// The part dialog judges every device it chooses for (doc 07 §4.3): the one-pole breaker offered to -102F1 and the
/// two-pole -101F1 fits the first and not the second, so it does not fit and says for how many and names the first it
/// does not fit; "Only parts that fit" keeps it, since it fits one, and hides a part of another kind, which fits none.
#[test]
fn an_offer_for_several_devices_is_judged_on_each() {
    use ew_api::{OfferView, PartOfferQuery};
    let s = common::sample();
    let one = s.resolve_reference("tag:-102F1").expect("-102F1");
    let two = s.resolve_reference("tag:-101F1").expect("-101F1");
    let query = |text: &str, fits_only: bool| PartOfferQuery {
        text: text.to_owned(),
        function: s.main_function(one).map(|f| f.0),
        devices: vec![one, two],
        fits_only,
        view: OfferView::All,
        ..PartOfferQuery::default()
    };
    let offers = s.part_offers(&query("EW-GEN-MCB-1", false));
    let offer = offers
        .iter()
        .find(|o| o.part_number == "EW-GEN-MCB-1")
        .expect("the one-pole breaker");
    let m = offer.misfit.as_ref().expect("it does not fit -101F1");
    assert_eq!((m.device, m.count, m.of), (two, 1, 2));
    let text = s.offer_misfit_text(m).to_string();
    assert!(text.starts_with("Does not fit 1 of 2; -101F1: It has no pin "), "{text}");
    let kept = s.part_offers(&query("EW-GEN-MCB-1", true));
    assert!(kept.iter().any(|o| o.part_number == "EW-GEN-MCB-1"), "it fits one of them");
    let hidden = s.part_offers(&query("EW-GEN-CON-1", true));
    assert!(hidden.iter().all(|o| o.part_number != "EW-GEN-CON-1"), "a contactor fits neither");
}
