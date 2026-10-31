// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The typed part, manufacturer and macro data of WP-S1.19 part 1 (D2, D15, ADR-0011 amendment 4): the fixture
//! copies of `common::lines::GOLDEN_LIB` parse into the typed members, a file written before part 1 (the
//! members in `extra`, byte-wise) reads them typed, the project's `basic_project` round-trips, and the kind
//! registry gives `macro_instance` rank 46 in its page's file while `placeholder_group` stays reserved.

use crate::common;

use common::fill;
use common::lines::{GOLDEN, GOLDEN_LIB};
use ew_schema::{
    AlternativeKind, ClampType, ContactKind, FileRule, Kind, KindStatus, LibDocument, MacroKind, MountKind,
    PartKind, Record, RepType, VarType,
};
use ew_units::{Nm, Vec2Nm};

fn lib(needle: &str) -> LibDocument {
    let mut found = GOLDEN_LIB.iter().filter(|t| t.contains(needle));
    let template = found
        .next()
        .unwrap_or_else(|| panic!("no golden document with {needle}"));
    assert!(found.next().is_none(), "{needle} is not unique");
    let text = fill(template);
    let doc: LibDocument = serde_json::from_str(&text).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), text, "not canonical");
    doc
}

fn part(needle: &str) -> ew_schema::Part {
    match lib(needle) {
        LibDocument::Part(c) => c.item,
        other => panic!("not a part: {:?}", other.kind()),
    }
}

fn nm(mm: i64) -> Nm {
    Nm::from_mm(mm)
}

#[test]
fn the_mech_fixture_types_dimensions_mounting_and_offset() {
    let p = part(r#""id":"@PART_MECH""#);
    let mech = p.mech.expect("mech");
    assert_eq!(
        (mech.width, mech.height, mech.depth),
        (Some(nm(60)), Some(nm(130)), Some(nm(125)))
    );
    assert_eq!(mech.mount_kind, Some(MountKind::Ts35x7_5));
    // FT-604: the rail offset is a vector (`Vec2Nm`), written `[dx, dy]` exactly as a `Pt2`.
    let offset: Option<Vec2Nm> = mech.rail_offset;
    assert_eq!(offset, Some(Vec2Nm::new(nm(30), nm(65))));
    assert_eq!(serde_json::to_string(&offset).unwrap(), "[30000000,65000000]");
    let clearance = mech.clearance.expect("clearance");
    assert_eq!((clearance.top, clearance.bottom), (nm(50), nm(50)));
    assert_eq!(
        (clearance.left, clearance.right, clearance.front),
        (Nm::ZERO, Nm::ZERO, Nm::ZERO)
    );
    assert_eq!(mech.front_2d.len(), 1);
    assert_eq!(mech.front_2d[0].len(), 4);
    assert_eq!(mech.mass, Some(1100));
    assert_eq!(p.alternatives.len(), 1);
    assert_eq!(p.alternatives[0].kind, AlternativeKind::Successor);
    assert_eq!(p.macros.len(), 1);
    assert_eq!(p.macros[0].rep_type, RepType::MultiLine);
    assert!(p.extra.is_empty() && mech.extra.is_empty());
}

#[test]
fn the_twin_terminal_fixture_types_the_clamp_members() {
    let p = part(r#""id":"@PART_TWIN""#);
    assert_eq!(p.kind, PartKind::Terminal);
    let cp = &p.cp_pattern[0];
    assert_eq!(cp.clamp_type, Some(ClampType::PushIn));
    assert_eq!(
        (
            cp.csa_min.map(|d| d.to_string()),
            cp.csa_max.map(|d| d.to_string())
        ),
        (Some("0.14".to_owned()), Some("4".to_owned()))
    );
    assert_eq!(cp.max_conductors, Some(2));
    assert!(cp.twin_ferrule);
    assert!(!p.cp_pattern[1].twin_ferrule);
    assert_eq!(p.cp_pattern[1].max_conductors, None);
}

#[test]
fn the_contactor_fixture_types_two_contacts() {
    let p = part(r#""id":"@PART_CONT""#);
    let kinds: Vec<&ContactKind> = p.contacts.iter().map(|c| &c.kind).collect();
    assert_eq!(kinds, [&ContactKind::No, &ContactKind::Nc]);
    assert_eq!(
        p.contacts[0].cps.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
        ["13", "14"]
    );
    assert_eq!((p.contacts[0].seq, p.contacts[1].seq), (Some(1), Some(2)));
    assert_eq!(
        p.contacts[1].special.as_ref().map(|s| s.as_str()),
        Some("early break")
    );
}

#[test]
fn the_carrier_fixtures_type_their_sections() {
    let encl = part(r#""id":"@PART_ENCL""#)
        .sections
        .enclosure
        .expect("enclosure");
    assert_eq!(
        (encl.width, encl.height, encl.depth, encl.wall),
        (
            Some(nm(600)),
            Some(nm(600)),
            Some(nm(250)),
            Some(Nm::from_um(1500))
        )
    );
    assert!(encl.plate.is_some());
    let duct = part(r#""id":"@PART_DUCT""#).sections.duct.expect("duct");
    assert_eq!(
        (duct.inner_width, duct.inner_height, duct.slot_pitch),
        (Some(nm(56)), Some(nm(76)), Some(Nm::from_um(12500)))
    );
    let rail = part(r#""id":"@PART_RAIL""#).sections.rail.expect("rail");
    assert_eq!(rail.profile, MountKind::Ts35x7_5);
    assert_eq!(rail.stock_length, Some(nm(2000)));
    let plate = part(r#""id":"@PART_PLATE""#)
        .sections
        .mounting_plate
        .expect("mounting plate");
    assert_eq!(plate.thickness, Some(nm(3)));
}

#[test]
fn the_configured_fixture_types_its_components() {
    let p = part(r#""id":"@PART_CONF""#);
    assert_eq!(p.kind, PartKind::Configured);
    let conf = p.configured.expect("configured");
    assert_eq!(conf.order_code.as_str(), "K-2C-24VDC+2xPT");
    let qty: Vec<String> = conf.components.iter().map(|c| c.qty.to_string()).collect();
    assert_eq!(qty, ["1", "2"]);
}

#[test]
fn the_manufacturer_and_macro_documents_are_typed() {
    let LibDocument::Manufacturer(m) = lib(r#""$k":"manufacturer""#) else {
        panic!("a manufacturer");
    };
    assert_eq!(m.item.key.as_str(), "EATON");
    assert_eq!(m.item.aliases.len(), 1);
    assert_eq!(m.item.brands.len(), 1);
    let LibDocument::Macro(mac) = lib(r#""$k":"macro""#) else {
        panic!("a macro");
    };
    let mac = mac.item;
    assert_eq!(mac.kind, MacroKind::Window);
    assert_eq!(mac.variants.len(), 1);
    assert_eq!(mac.variants[0].rep_type, RepType::MultiLine);
    assert!(mac.variants[0].fragment.is_object());
    assert_eq!(mac.variables[0].ty, VarType::TagCounter);
    assert_eq!(mac.variables[0].bindings[0].field, "device.tag.counter");
    assert_eq!(mac.variables[1].ty, VarType::Bool);
    assert_eq!(mac.groups[0].include.as_deref(), Some("with_fuses"));
    assert_eq!(mac.value_sets[0].values.len(), 2);
    assert!(mac.thumbnail.is_some());
}

#[test]
fn the_macro_instance_line_is_typed_and_anchored_on_its_page() {
    let template = GOLDEN
        .iter()
        .find(|t| t.contains(r#""$k":"macro_instance""#))
        .expect("a macro_instance line");
    let line = fill(template);
    let record: Record = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
    let Record::MacroInstance(mi) = &record else {
        panic!("a macro instance");
    };
    assert_eq!(mi.variant.as_str(), "A");
    assert_eq!(mi.value_set.as_ref().map(|v| v.as_str()), Some("1"));
    assert_eq!(mi.members.len(), 2);
    assert_eq!(record.kind(), Some(Kind::MacroInstance));
}

/// L1-3, schema half: a part written before part 1 carries `mech` (and the other new members) in `extra`,
/// after the typed members in byte-wise key order, with `mech`'s own members byte-wise too. It reads typed,
/// nothing stays in `extra`, and the canonical write moves the members to doc 07 §2.5's order.
#[test]
fn a_part_written_before_part_one_reads_its_members_typed() {
    let old = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.3.0","sha256":"@HASH","#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","#,
        r#""sections":{"duct":{"inner_width":56000000}},"#,
        r#""alternatives":[{"kind":"successor","part":{"item":"@ITEM_PSU2","library":"@LIB","version_req":"^1"}}],"#,
        r#""contacts":[{"cps":["13","14"],"kind":"no"}],"#,
        r#""mech":{"clearance":{"top":50000000},"depth":125000000,"height":130000000,"mass":1100,"mount_kind":"ts35x7_5","rail_offset":[30000000,65000000],"width":60000000}}"#
    ));
    let doc: LibDocument = serde_json::from_str(&old).unwrap();
    let LibDocument::Part(copy) = &doc else {
        panic!("a part");
    };
    let p = &copy.item;
    assert!(p.extra.is_empty(), "left in extra: {:?}", p.extra);
    assert!(p.sections.extra.is_empty());
    let mech = p.mech.as_ref().expect("mech typed");
    assert_eq!(mech.width, Some(nm(60)));
    assert_eq!(mech.mount_kind, Some(MountKind::Ts35x7_5));
    assert_eq!(p.contacts.len(), 1);
    assert_eq!(p.alternatives.len(), 1);
    assert_eq!(p.sections.duct.as_ref().and_then(|d| d.inner_width), Some(nm(56)));

    let canonical = serde_json::to_string(&doc).unwrap();
    assert_ne!(
        canonical, old,
        "the canonical order differs from the byte-wise one"
    );
    let expected = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.3.0","sha256":"@HASH","#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","#,
        r#""contacts":[{"kind":"no","cps":["13","14"]}],"#,
        r#""mech":{"width":60000000,"height":130000000,"depth":125000000,"mount_kind":"ts35x7_5","rail_offset":[30000000,65000000],"clearance":{"top":50000000},"mass":1100},"#,
        r#""sections":{"duct":{"inner_width":56000000}},"#,
        r#""alternatives":[{"part":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^1"},"kind":"successor"}]}"#
    ));
    assert_eq!(canonical, expected);
    // The rewritten line reads back to the same typed value.
    let again: LibDocument = serde_json::from_str(&canonical).unwrap();
    assert_eq!(again, doc);
}

/// A value of a newer minor in the new enums is kept verbatim (ADR-0011 amendment 1).
#[test]
fn unknown_enum_values_of_the_new_members_survive() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"1.3.0","sha256":"@HASH","#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"2904602","#,
        r#""cp_pattern":[{"idx":0,"designation":"1","clamp_type":"lever"}],"#,
        r#""mech":{"mount_kind":"ts75"},"#,
        r#""alternatives":[{"part":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^1"},"kind":"customer_approved"}]}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line);
    let LibDocument::Part(copy) = &doc else {
        panic!("a part");
    };
    assert!(matches!(
        copy.item.cp_pattern[0].clamp_type,
        Some(ClampType::Unknown(_))
    ));
    assert!(matches!(
        copy.item.mech.as_ref().and_then(|m| m.mount_kind.clone()),
        Some(MountKind::Unknown(_))
    ));
    assert!(matches!(
        copy.item.alternatives[0].kind,
        AlternativeKind::Unknown(_)
    ));
}

#[test]
fn the_project_carries_its_basic_project_reference() {
    let template = GOLDEN
        .iter()
        .find(|t| t.contains(r#""$k":"project""#))
        .expect("a project line");
    let base = fill(template);
    let with = format!(
        "{},\"basic_project\":{{\"id\":\"{}\",\"generation\":7,\"zz_newer\":true}}}}",
        &base[..base.len() - 1],
        fill("@BASIC_PRJ")
    );
    let record: Record = serde_json::from_str(&with).unwrap();
    assert_eq!(serde_json::to_string(&record).unwrap(), with);
    let Record::Project(p) = record else {
        panic!("a project");
    };
    let basic = p.basic_project.expect("basic_project");
    assert_eq!(basic.generation, 7);
    assert_eq!(basic.extra.len(), 1);
    // Absent, it writes nothing: the golden line is unchanged.
    let plain: Record = serde_json::from_str(&base).unwrap();
    assert_eq!(serde_json::to_string(&plain).unwrap(), base);
}

#[test]
fn the_registry_types_the_three_kinds_and_keeps_placeholder_group_reserved() {
    assert_eq!(Kind::MacroInstance.rank(), Some(46));
    assert_eq!(Kind::MacroInstance.status(), KindStatus::Implemented);
    assert_eq!(Kind::MacroInstance.file_rule(), FileRule::PageFile);
    let at_46: Vec<Kind> = Kind::ALL
        .iter()
        .copied()
        .filter(|k| k.rank() == Some(46))
        .collect();
    assert_eq!(at_46, [Kind::MacroInstance], "rank 46 is used once");
    assert_eq!(Kind::PlaceholderGroup.rank(), Some(47));
    assert_eq!(Kind::PlaceholderGroup.status(), KindStatus::Reserved);
    for k in [Kind::Macro, Kind::Manufacturer] {
        assert_eq!(k.status(), KindStatus::Implemented, "{}", k.name());
        assert_eq!(k.rank(), None, "{} is a library document", k.name());
    }
}
