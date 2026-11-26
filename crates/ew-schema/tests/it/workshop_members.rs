// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members of the parts for the workshop (doc 07 §2.5): a part's accessories and variants, the wire, ferrule and
//! marker sections, a device's accessory choices and the project's workshop settings read typed and write back in
//! their member order; a newer enum value is kept.

use crate::common;

use common::fill;
use ew_schema::{AccessoryKind, AccessoryPer, Device, LibDocument, MarkTarget, Part, ProjectSettings};
use ew_units::Nm;

/// A terminal part with an end plate per strip and an end bracket per rail group, a blue variant, written in member
/// order: `accessories` after `sections`, `variants` after `alternatives`.
const TERMINAL: &str = concat!(
    r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"0.1.3","sha256":"@HASH","#,
    r#""kind":"terminal","manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"#,
    r#""part_number":"EW-GEN-TRM-1","#,
    r#""tech_data":[{"key":"colour","value":"GY"}],"#,
    r#""accessories":[{"part":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^0.1"},"qty":"1","per":"strip","kind":"end_plate"},"#,
    r#"{"part":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"qty":"1","per":"rail_group","kind":"end_clamp","optional":true}],"#,
    r#""alternatives":[{"part":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^1"},"kind":"successor"}],"#,
    r#""variants":[{"key":"BU","name":{"en":"Blue","nl":"Blauw"},"part_number":"EW-GEN-TRM-1-BU","#,
    r#""description":{"en":"Feed-through terminal, blue"},"tech_data":[{"key":"colour","value":"BU"}]}]}"#
);

fn part(line: &str) -> (LibDocument, Part) {
    let doc: LibDocument = serde_json::from_str(line).unwrap();
    let LibDocument::Part(copy) = &doc else {
        panic!("a part");
    };
    let item = copy.item.clone();
    (doc, item)
}

#[test]
fn a_part_reads_its_accessories_and_variants_typed() {
    let line = fill(TERMINAL);
    let (doc, p) = part(&line);
    assert!(p.extra.is_empty(), "left in extra: {:?}", p.extra);
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    let per: Vec<_> = p
        .accessories
        .iter()
        .map(|a| (&a.per, &a.kind, a.optional))
        .collect();
    assert_eq!(
        per,
        [
            (&AccessoryPer::Strip, &Some(AccessoryKind::EndPlate), false),
            (&AccessoryPer::RailGroup, &Some(AccessoryKind::EndClamp), true),
        ]
    );
    let blue = p.variant("BU").expect("the variant BU");
    assert_eq!(
        blue.part_number.as_ref().map(|n| n.as_str()),
        Some("EW-GEN-TRM-1-BU")
    );
    assert!(p.variant("RD").is_none());
}

/// A variant gives at least its own part number, order number or description.
#[test]
fn a_variant_has_its_own_number_by_its_number_order_number_or_description() {
    let (_, p) = part(&fill(TERMINAL));
    let mut v = p.variant("BU").unwrap().clone();
    assert!(v.has_own_number());
    v.part_number = None;
    assert!(v.has_own_number(), "the description tells it apart");
    v.description = Default::default();
    assert!(!v.has_own_number());
    v.order_number = Some("1234".parse().unwrap());
    assert!(v.has_own_number());
}

/// The sections of the parts a wire consumes are typed after `fluid`, each in its member order.
#[test]
fn the_wire_ferrule_and_marker_sections_are_typed() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"0.1.1","sha256":"@HASH","#,
        r#""kind":"ferrule","manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"#,
        r#""part_number":"EW-GEN-FER-2X0.75","#,
        r#""sections":{"wire":{"csa":"0.75","colour":["BK"],"class":5},"#,
        r#""ferrule":{"csa_min":"0.75","csa_max":"0.75","twin":true,"insulated":true,"sleeve":8000000,"colour":["GY"],"stud":"M4"},"#,
        r#""marker":{"marks":"terminal","lines":1,"chars":4}}}"#
    ));
    let (doc, p) = part(&line);
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    assert!(p.sections.extra.is_empty());
    let wire = p.sections.wire.as_ref().expect("wire");
    assert_eq!((wire.csa.to_string(), wire.class), ("0.75".to_owned(), Some(5)));
    let ferrule = p.sections.ferrule.as_ref().expect("ferrule");
    assert!(ferrule.twin && ferrule.insulated);
    assert_eq!(ferrule.sleeve, Some(Nm::from_mm(8)));
    assert_eq!(ferrule.stud.as_ref().map(|s| s.as_str()), Some("M4"));
    let marker = p.sections.marker.as_ref().expect("marker");
    assert_eq!(
        (&marker.marks, marker.lines, marker.chars),
        (&MarkTarget::Terminal, Some(1), Some(4))
    );
}

/// A value of a newer minor in `per` and `marks` is kept verbatim (ADR-0011 amendment 1).
#[test]
fn unknown_accessory_per_and_mark_target_survive() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"0.1.1","sha256":"@HASH","#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"part_number":"X","#,
        r#""sections":{"marker":{"marks":"tube"}},"#,
        r#""accessories":[{"part":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^0.1"},"qty":"2","per":"cabinet"}]}"#
    ));
    let (doc, p) = part(&line);
    assert_eq!(serde_json::to_string(&doc).unwrap(), line);
    assert!(matches!(p.accessories[0].per, AccessoryPer::Unknown(_)));
    assert!(matches!(
        p.sections.marker.as_ref().map(|m| &m.marks),
        Some(MarkTarget::Unknown(_))
    ));
}

/// A device's accessory choices follow its part uses; a choice without a part takes the accessory off.
#[test]
fn a_device_reads_its_accessory_choices() {
    let json = concat!(
        r#"{"id":"0190a000-0000-7000-8000-000000000001","tag":{"letter":"X","counter":1},"#,
        r#""parts":[{"part":{"copy":"0190a000-0000-7000-8000-000000000002"},"qty":"1"}],"#,
        r#""accessory_choices":[{"of":"0190a000-0000-7000-8000-000000000003"},"#,
        r#"{"of":"0190a000-0000-7000-8000-000000000004","part":{"copy":"0190a000-0000-7000-8000-000000000005"}}]}"#
    );
    let d: Device = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&d).unwrap(), json);
    assert!(d.accessory_choices[0].part.is_none());
    assert!(d.accessory_choices[1].part.is_some());
}

/// The workshop settings: the stock in its order and the cut increment; absent settings are not written.
#[test]
fn the_project_settings_hold_the_workshop_stock() {
    let json = concat!(
        r#"{"workshop":{"stock":[{"copy":"0190a000-0000-7000-8000-000000000002"},"#,
        r#"{"copy":"0190a000-0000-7000-8000-000000000001"}],"cut_increment":5000000}}"#
    );
    let s: ProjectSettings = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&s).unwrap(), json);
    assert_eq!(s.workshop.cut_increment, Some(Nm::from_mm(5)));
    assert_eq!(serde_json::to_string(&ProjectSettings::default()).unwrap(), "{}");
}
