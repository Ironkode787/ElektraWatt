// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The layout kinds typed within format 0.1 (WP-S4.01 D3, LM-1; ADR-0011 amendment 4): `layout_space`,
//! `enclosure`, `mounting_surface`, `carrier` and `placement` round-trip byte for byte with every typed member, the
//! members and variants of later packages survive a load and a save verbatim, a malformed known variant is kept by
//! the loader (EW-CORE-017), and each record names the anchor its layout file is resolved from.

use crate::common;

use common::fill;
use ew_schema::{
    AnchorRef, CarrierKind, CarrierParams, CarrierRef, FileRule, Kind, KindStatus, MountKind, Record,
    SurfaceKind,
};
use ew_units::{Nm, Quat9};

fn canonical(template: &str) -> Record {
    let line = fill(template);
    let record: Record = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(serde_json::to_string(&record).unwrap(), line, "not canonical");
    record
}

const MM: i64 = 1_000_000;

#[test]
fn the_layout_kinds_are_implemented_layout_file_kinds() {
    for (kind, rank) in [
        (Kind::LayoutSpace, 50),
        (Kind::Enclosure, 51),
        (Kind::MountingSurface, 52),
        (Kind::Carrier, 53),
        (Kind::Placement, 54),
    ] {
        assert_eq!(kind.status(), KindStatus::Implemented, "{kind}");
        assert_eq!(kind.rank(), Some(rank));
        assert_eq!(kind.schema_version(), 1);
        assert_eq!(kind.file_rule(), FileRule::LayoutFile);
    }
    // Routing and machining kinds stay reserved (WP-S4.05, WP-S4.07).
    for kind in [
        Kind::RouteOverride,
        Kind::CommittedRoute,
        Kind::RoutingPath,
        Kind::SurfaceFeature,
    ] {
        assert_eq!(kind.status(), KindStatus::Reserved, "{kind}");
    }
}

#[test]
fn golden_layout_records_are_typed() {
    let Record::LayoutSpace(space) = canonical(
        r#"{"$k":"layout_space","id":"@SPACE_JE2416","location":"@LOC_JE2416","name":"+J-E-2416","install":{"wall_behind":true}}"#,
    ) else {
        panic!("a layout space")
    };
    assert_eq!(space.name.as_ref().map(|n| n.as_str()), Some("+J-E-2416"));
    assert!(space.extra.contains_key("install"), "install stays in extra");

    let Record::MountingSurface(surface) = canonical(
        r#"{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"plate","device":"@DEV_PLATE","frame":{"t":[50000000,50000000,0],"q":[1000000000,0,0,0]},"usable":[[0,0],[650000000,0],[650000000,900000000],[0,900000000]],"pe_bond":true,"max_depth":250000000}"#,
    ) else {
        panic!("a surface")
    };
    assert_eq!(surface.kind, SurfaceKind::Plate);
    assert_eq!(surface.frame.t, [Nm(50 * MM), Nm(50 * MM), Nm(0)]);
    assert_eq!(surface.usable.len(), 4);
    assert!(surface.pe_bond && surface.device.is_some());

    let Record::Carrier(rail) = canonical(
        r#"{"$k":"carrier","id":"@CAR_R1","surface":"@SURF_PLATE","kind":"rail","part":{"copy":"@PART_RAIL"},"path":[[40000000,740000000],[610000000,740000000]],"params":{"rail":{"profile":"ts35x7_5","perforated":true}},"pe_bond":true}"#,
    ) else {
        panic!("a rail")
    };
    assert_eq!(rail.kind, CarrierKind::Rail);
    let CarrierParams::Rail(params) = &rail.params else {
        panic!("rail params")
    };
    assert_eq!(params.profile, MountKind::Ts35x7_5);
    assert!(params.extra.contains_key("perforated"));

    let Record::Carrier(duct) = canonical(
        r#"{"$k":"carrier","id":"@CAR_D1","surface":"@SURF_PLATE","kind":"duct","path":[[40000000,860000000],[610000000,860000000],[610000000,340000000]],"params":{"duct":{"width":40000000,"height":60000000,"slot_pitch":12500000,"cover":true,"finger_w":6000000}}}"#,
    ) else {
        panic!("a duct")
    };
    let CarrierParams::Duct(params) = &duct.params else {
        panic!("duct params")
    };
    assert_eq!((params.width, params.height), (Nm(40 * MM), Nm(60 * MM)));
    assert_eq!(params.slot_pitch, Some(Nm(12_500_000)));
    assert_eq!(params.cover, Some(true));
    assert!(duct.part.is_none() && !duct.pe_bond);

    let Record::Placement(flipped) = canonical(
        r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","function":"@FN_G1","part":{"copy":"@PART_PSU"},"carrier":{"carrier":"@CAR_R1"},"pose":{"t":[96000000,0,0],"q":[0,0,0,1000000000]},"locked":true,"variant":"lying"}"#,
    ) else {
        panic!("a placement")
    };
    assert!(matches!(flipped.carrier, CarrierRef::Carrier(_)));
    assert_eq!(flipped.pose.q, Quat9::new(0, 0, 0, Quat9::SCALE).unwrap());
    assert!(flipped.locked && flipped.function.is_some() && flipped.part.is_some());
    assert!(flipped.extra.contains_key("variant"));
}

/// The defaults are not written: a placement with only its required members is the line of doc 12 §2.3.
#[test]
fn a_minimal_placement_is_the_line_of_doc_12() {
    let Record::Placement(p) = canonical(
        r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"carrier":"@CAR_R1"},"pose":{"t":[96000000,0,0],"q":[1000000000,0,0,0]}}"#,
    ) else {
        panic!("a placement")
    };
    assert!(!p.locked && p.part.is_none() && p.function.is_none() && p.extra.is_empty());
    let Record::Enclosure(e) = canonical(
        r#"{"$k":"enclosure","id":"@ENCL_UC1","space":"@SPACE_JE2416","device":"@DEV_UC1","pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#,
    ) else {
        panic!("an enclosure")
    };
    assert_eq!(e.pose, ew_units::Pose3::IDENTITY);
    canonical(r#"{"$k":"layout_space","id":"@SPACE_JE2416","location":"@LOC_JE2416"}"#);
}

/// LM-1: a surface kind, a carrier kind and parameters of a later package, a piggy-back `CarrierRef`, and newer
/// members at every depth survive a load and a save byte for byte.
#[test]
fn newer_layout_values_survive_a_load_and_a_save_verbatim() {
    let Record::MountingSurface(s) = canonical(
        r#"{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"door_inside","frame":{"t":[0,0,0],"q":[1000000000,0,0,0]},"hinge":"left"}"#,
    ) else {
        panic!("a surface")
    };
    assert!(matches!(s.kind, SurfaceKind::Unknown(_)) && s.usable.is_empty());
    let Record::Carrier(c) = canonical(
        r#"{"$k":"carrier","id":"@CAR_R1","surface":"@SURF_PLATE","kind":"busbar","path":[[0,0],[600000000,0]],"params":{"busbar":{"phases":["L1","L2","L3"],"pitch":60000000}}}"#,
    ) else {
        panic!("a carrier")
    };
    assert!(matches!(c.kind, CarrierKind::Unknown(_)));
    assert!(matches!(c.params, CarrierParams::Unknown(_)));
    let Record::Placement(p) = canonical(
        r#"{"$k":"placement","id":"@PL_S51","device":"@DEV_S51","carrier":{"placement":"@PL_G1"},"pose":{"t":[0,0,0],"q":[1000000000,0,0,0]},"segment":{"from_seq":1,"to_seq":4}}"#,
    ) else {
        panic!("a placement")
    };
    assert!(matches!(p.carrier, CarrierRef::Unknown(_)));
    assert_eq!(p.extra.len(), 1);
    let Record::Carrier(c) = canonical(
        r#"{"$k":"carrier","id":"@CAR_D1","surface":"@SURF_PLATE","kind":"duct","path":[[0,0],[0,100000000]],"params":{"duct":{"width":40000000,"height":60000000,"joints":["butt"]}},"class_set":["power"]}"#,
    ) else {
        panic!("a duct")
    };
    assert_eq!(c.extra.len(), 1);
}

/// A known variant whose payload does not parse (a duct without its height) is refused by the strict reader and
/// kept verbatim by the loader, which reports it (EW-CORE-017).
#[test]
fn a_malformed_duct_is_kept_by_the_loader() {
    let line = fill(
        r#"{"$k":"carrier","id":"@CAR_D1","surface":"@SURF_PLATE","kind":"duct","path":[[0,0],[0,100000000]],"params":{"duct":{"width":40000000}}}"#,
    );
    assert!(serde_json::from_str::<Record>(&line).is_err());
    let (record, malformed) = Record::load(ew_schema::parse_value(&line).unwrap()).unwrap();
    assert_eq!(malformed.len(), 1, "{malformed:?}");
    assert_eq!(malformed[0].variant, "duct");
    let Record::Carrier(c) = &record else {
        panic!("a carrier")
    };
    assert!(matches!(c.params, CarrierParams::Unknown(_)));
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
}

/// The required members are refused when missing, and a pose that is not a sign-normalised unit quaternion is
/// refused at its path.
#[test]
fn missing_members_and_bad_poses_are_refused() {
    for (template, path) in [
        (r#"{"$k":"layout_space","id":"@SPACE_JE2416"}"#, ""),
        (
            r#"{"$k":"enclosure","id":"@ENCL_UC1","space":"@SPACE_JE2416","device":"@DEV_UC1"}"#,
            "",
        ),
        (
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#,
            "",
        ),
        (
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"carrier":"@CAR_R1"},"pose":{"t":[0,0,0],"q":[0,0,0,-1000000000]}}"#,
            "/pose",
        ),
        (
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"carrier":"@CAR_R1"},"pose":{"t":[0,0],"q":[1000000000,0,0,0]}}"#,
            "/pose",
        ),
        (
            r#"{"$k":"carrier","id":"@CAR_R1","surface":"@SURF_PLATE","kind":"rail","path":[[0,0],[1,0]]}"#,
            "",
        ),
    ] {
        let err = Record::from_json(&fill(template)).unwrap_err();
        assert!(err.path().unwrap_or("").starts_with(path), "{template}: {err}");
    }
}

/// Each layout record names where its file's space comes from (doc 05 §2.2): a space is its own anchor, an
/// enclosure names its space, a surface its enclosure, a carrier its surface, a placement its carrier or surface.
#[test]
fn layout_records_name_their_anchor() {
    let id = |token: &str| fill(token).parse::<ew_ids::Uuid>().unwrap();
    let anchor = |template: &str| canonical(template).anchor_ref();
    assert_eq!(
        anchor(r#"{"$k":"layout_space","id":"@SPACE_JE2416","location":"@LOC_JE2416"}"#),
        AnchorRef::Direct(id("@SPACE_JE2416"))
    );
    assert_eq!(
        anchor(
            r#"{"$k":"enclosure","id":"@ENCL_UC1","space":"@SPACE_JE2416","device":"@DEV_UC1","pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#
        ),
        AnchorRef::Direct(id("@SPACE_JE2416"))
    );
    assert!(matches!(
        anchor(
            r#"{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"plate","frame":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#
        ),
        AnchorRef::SpaceOfEnclosure(e) if e.as_uuid() == id("@ENCL_UC1")
    ));
    assert!(matches!(
        anchor(
            r#"{"$k":"carrier","id":"@CAR_R1","surface":"@SURF_PLATE","kind":"rail","path":[[0,0],[1,0]],"params":{"rail":{"profile":"ts32"}}}"#
        ),
        AnchorRef::SpaceOfSurface(s) if s.as_uuid() == id("@SURF_PLATE")
    ));
    assert!(matches!(
        anchor(
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"carrier":"@CAR_R1"},"pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#
        ),
        AnchorRef::SpaceOfCarrier(c) if c.as_uuid() == id("@CAR_R1")
    ));
    assert!(matches!(
        anchor(
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"surface":"@SURF_PLATE"},"pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#
        ),
        AnchorRef::SpaceOfSurface(s) if s.as_uuid() == id("@SURF_PLATE")
    ));
    assert_eq!(
        anchor(
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"placement":"@PL_S51"},"pose":{"t":[0,0,0],"q":[1000000000,0,0,0]}}"#
        ),
        AnchorRef::Unknown
    );
}

/// A layout record written while the kinds were reserved (kept verbatim, members in byte order) reads as the typed
/// record; only its member order changes when it is written back.
#[test]
fn a_record_written_while_reserved_reads_typed() {
    let old = fill(
        r#"{"$k":"placement","carrier":{"carrier":"@CAR_R1"},"device":"@DEV_G1","id":"@PL_G1","pose":{"q":[1000000000,0,0,0],"t":[96000000,0,0]}}"#,
    );
    let record = Record::from_json(&old).unwrap();
    assert_eq!(record.kind(), Some(Kind::Placement));
    assert_eq!(
        serde_json::to_string(&record).unwrap(),
        fill(
            r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"carrier":"@CAR_R1"},"pose":{"t":[96000000,0,0],"q":[1000000000,0,0,0]}}"#
        )
    );
}
