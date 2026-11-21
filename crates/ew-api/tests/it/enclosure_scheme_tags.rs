// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1899: an enclosure made from a part and its mounting plate take the numbering scheme's tags, not frozen ones, so
//! the check reports no "frozen, deviates from scheme" (EW-NUM-001) for them right after they are made. The enclosure's
//! device, which no page draws, prints no page in its tag; its counter is the next one no other `UC` tag holds.

use crate::common;

use ew_api::AnyCommand;
use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::{DeviceTag, Q};
use ew_schema::{Device, Enclosure, MountingSurface};
use serde_json::json;

/// The tag of `device` as the scheme derives it, and whether it is frozen.
fn tag(q: &Q, device: Id<Device>) -> (String, bool) {
    let product = q
        .get::<DeviceTag>(device)
        .as_ref()
        .as_ref()
        .map(|t| t.product.clone())
        .expect("a tag");
    let frozen = q.devices().get(device).expect("the device").tag.frozen.is_some();
    (product, frozen)
}

#[test]
fn a_new_enclosure_and_its_plate_take_the_schemes_tags() {
    let mut s = common::sample();
    let field: Uuid = s
        .query()
        .structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == "FLD")
        .map(|(id, _)| id.as_uuid())
        .expect("the sample's location +FLD");
    let part = s
        .layout_part_offers("enclosure")
        .into_iter()
        .find(|o| o.number == "EW-GEN-ENC-1" && o.copies.is_empty())
        .expect("the project's enclosure part")
        .part;
    let before: Vec<Id<Enclosure>> = s.query().table::<Enclosure>().ids().collect();
    s.execute(&AnyCommand {
        name: "layout.enclosure.create_from_part".into(),
        params: json!({"location": field.to_string(), "part": part.to_string()}),
    })
    .expect("the enclosure");

    let q = s.query();
    let (id, enclosure) = q
        .table::<Enclosure>()
        .iter()
        .find(|(id, _)| !before.contains(id))
        .map(|(id, e)| (id, e.device))
        .expect("the new enclosure");
    let plate = q
        .table::<MountingSurface>()
        .iter()
        .find(|(_, m)| m.enclosure == id)
        .and_then(|(_, m)| m.device)
        .expect("its plate device");
    // The sample's own enclosure is UC1, so the new one is UC2; the plate is its child U1. Neither is frozen.
    assert_eq!(tag(&q, enclosure), ("UC2".to_owned(), false));
    assert_eq!(tag(&q, plate), ("UC2-U1".to_owned(), false));
    drop(q);

    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let deviating: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-NUM-001" && f.is_open() && f.subject.starts_with("+FLD"))
        .map(|f| f.subject.as_str())
        .collect();
    assert_eq!(
        deviating,
        Vec::<&str>::new(),
        "no tag of +FLD's enclosure deviates"
    );
}
