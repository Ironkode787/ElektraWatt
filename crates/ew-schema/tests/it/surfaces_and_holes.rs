// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The cabinet's surfaces and their holes (doc 12 §2.2, §2.5, §2.8): the faces of a cabinet with a door's hinge, a
//! datum and an edge margin, the hole choices, the surface feature, a part's front shape and drilling pattern, the
//! enclosure's doors, a cable's and a gland's sections, a cable's chosen gland and the workshop's hole settings read
//! typed and write back in their member order; a value of a newer format is kept; and a hole's text form parses by
//! its kind.

use crate::common;

use common::fill;
use ew_schema::{
    AnchorRef, Corner, Cutout, Device, FileRule, FixingRule, FrontShape, Gland, Hinge, Hole, HoleKind,
    HoleParseError, Kind, KindStatus, LibDocument, MechData, PartKind, ProjectSettings, Record, Screw,
    SurfaceKind,
};
use ew_units::{Nm, Pt2};

const MM: i64 = 1_000_000;

fn canonical(template: &str) -> Record {
    let line = fill(template);
    let record: Record = serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(serde_json::to_string(&record).unwrap(), line, "not canonical");
    record
}

/// A hole kind's word is the member that names it in a hole's stored form, and names the kind back.
#[test]
fn a_hole_kinds_word_names_its_stored_form() {
    for (kind, text) in [
        (HoleKind::Round, "4.5"),
        (HoleKind::Fixing, "M5"),
        (HoleKind::Cutout, "22"),
        (HoleKind::Gland, "M20"),
        (HoleKind::Rect, "92 x 92"),
    ] {
        let json = serde_json::to_value(Hole::parse(kind, text).unwrap()).unwrap();
        let stored = json
            .as_object()
            .and_then(|o| o.keys().next().cloned())
            .or_else(|| json.as_str().map(str::to_owned))
            .unwrap();
        assert_eq!(stored, kind.word(), "{json}");
        assert_eq!(HoleKind::of_word(kind.word()), Some(kind));
    }
}

#[test]
fn the_surface_feature_is_an_implemented_layout_file_kind() {
    let kind = Kind::SurfaceFeature;
    assert_eq!(kind.status(), KindStatus::Implemented);
    assert_eq!(kind.rank(), Some(58));
    assert_eq!(kind.file_rule(), FileRule::LayoutFile);
}

/// A door with its hinge, datum and edge margin; every face of the cabinet reads as its kind.
#[test]
fn a_door_reads_its_hinge_datum_and_margin() {
    let Record::MountingSurface(door) = canonical(
        r#"{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"door","frame":{"t":[0,0,0],"q":[1000000000,0,0,0]},"hinge":"left","datum":"top_left","edge_margin":30000000}"#,
    ) else {
        panic!("a surface")
    };
    assert_eq!(door.kind, SurfaceKind::Door);
    assert_eq!(door.hinge, Some(Hinge::Left));
    assert_eq!(door.datum, Some(Corner::TopLeft));
    assert_eq!(door.edge_margin, Some(Nm(30 * MM)));
    for (word, kind) in [
        ("left", SurfaceKind::Left),
        ("right", SurfaceKind::Right),
        ("top", SurfaceKind::Top),
        ("bottom", SurfaceKind::Bottom),
    ] {
        let line = format!(
            r#"{{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"{word}","frame":{{"t":[0,0,0],"q":[1000000000,0,0,0]}}}}"#
        );
        let Record::MountingSurface(s) = canonical(&line) else {
            panic!("a surface")
        };
        assert_eq!(s.kind, kind);
    }
}

/// A newer surface kind, hinge and corner are kept verbatim.
#[test]
fn newer_surface_values_survive() {
    let Record::MountingSurface(s) = canonical(
        r#"{"$k":"mounting_surface","id":"@SURF_PLATE","enclosure":"@ENCL_UC1","kind":"gland_plate","frame":{"t":[0,0,0],"q":[1000000000,0,0,0]},"hinge":"top","datum":"centre"}"#,
    ) else {
        panic!("a surface")
    };
    assert!(matches!(s.kind, SurfaceKind::Unknown(_)));
    assert!(matches!(s.hinge, Some(Hinge::Unknown(_))));
    assert!(matches!(s.datum, Some(Corner::Unknown(_))));
}

/// A placement's and a carrier's hole choices: one moved, one left out.
#[test]
fn hole_choices_read_after_their_neighbours() {
    let Record::Placement(p) = canonical(
        r#"{"$k":"placement","id":"@PL_G1","device":"@DEV_G1","carrier":{"surface":"@SURF_PLATE"},"pose":{"t":[255000000,465000000,0],"q":[1000000000,0,0,0]},"label_at":[0,1000000],"hole_choices":[{"index":0,"offset":[2000000,0]},{"index":1,"omit":true}]}"#,
    ) else {
        panic!("a placement")
    };
    assert_eq!(p.hole_choices.len(), 2);
    assert_eq!(p.hole_choices[0].offset, Some(Pt2::new(Nm(2 * MM), Nm(0))));
    assert!(p.hole_choices[1].omit && p.hole_choices[1].offset.is_none());
    let Record::Carrier(c) = canonical(
        r#"{"$k":"carrier","id":"@CAR_R1","surface":"@SURF_PLATE","kind":"rail","path":[[0,0],[470000000,0]],"params":{"rail":{"profile":"ts35x7_5"}},"pe_bond":true,"hole_choices":[{"index":2,"omit":true}]}"#,
    ) else {
        panic!("a rail")
    };
    assert_eq!(c.hole_choices[0].index, 2);
}

/// A hole by hand and a cable's gland, each anchored at its surface's space; every form of a hole.
#[test]
fn a_surface_feature_reads_typed() {
    let Record::SurfaceFeature(f) = canonical(
        r#"{"$k":"surface_feature","id":"@PL_G1","surface":"@SURF_PLATE","at":[490000000,40000000],"hole":{"fixing":"m6"},"purpose":{"en":"Earth stud"}}"#,
    ) else {
        panic!("a feature")
    };
    assert_eq!(f.hole, Hole::Fixing(Screw::M6));
    assert!(f.cable.is_none() && !f.for_cable);
    assert!(matches!(
        Record::SurfaceFeature(f).anchor_ref(),
        AnchorRef::SpaceOfSurface(s) if s.as_uuid() == fill("@SURF_PLATE").parse::<ew_ids::Uuid>().unwrap()
    ));
    let Record::SurfaceFeature(g) = canonical(
        r#"{"$k":"surface_feature","id":"@PL_G1","surface":"@SURF_PLATE","at":[80000000,125000000],"hole":{"gland":"m20"},"cable":"@DEV_G1","for_cable":true}"#,
    ) else {
        panic!("a gland")
    };
    assert_eq!(g.hole, Hole::Gland(Gland::M20));
    assert!(g.for_cable && g.cable.is_some());
    for hole in [
        r#"{"round":{"d":6000000}}"#,
        r#"{"cutout":"d22"}"#,
        r#"{"rect":{"w":92000000,"h":92000000,"r":2000000}}"#,
        r#"{"rect":{"w":45000000,"h":45000000}}"#,
        r#"{"slot":{"l":12000000,"w":6000000}}"#,
        r#"{"fixing":"m10"}"#,
    ] {
        let h: Hole = serde_json::from_str(hole).unwrap();
        assert_eq!(serde_json::to_string(&h).unwrap(), hole);
    }
}

/// A part's front shape and drilling pattern follow its mass; the enclosure's doors, margin and depth follow its
/// plate; a cable's and a gland's sections follow the marker's; a gland part is of kind `gland`.
#[test]
fn a_part_reads_its_front_holes_doors_and_gland() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"0.1.1","sha256":"@HASH","#,
        r#""kind":"gland","manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"#,
        r#""part_number":"EW-GEN-GLD-M20","#,
        r#""mech":{"width":30000000,"height":30000000,"depth":45000000,"mount_kind":"door_cutout","mass":20,"#,
        r#""front_shape":"round","holes":[{"x":15000000,"y":15000000,"hole":{"cutout":"d22"}}]},"#,
        r#""sections":{"enclosure":{"width":600000000,"plate":{"library":"@LIB","item":"@ITEM_PSU2","version_req":"^0.1"},"#,
        r#""doors":[{"hinge":"left","width":300000000},{"hinge":"right"}],"edge_margin":30000000,"usable_depth":200000000},"#,
        r#""cable":{"outer_diameter":10500000},"#,
        r#""gland":{"thread":"m20","clamp_min":6000000,"clamp_max":12000000,"nut":27000000}}}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    let LibDocument::Part(copy) = &doc else {
        panic!("a part")
    };
    let p = &copy.item;
    assert_eq!(p.kind, PartKind::Gland);
    assert!(!p.kind.is_consumable(), "a gland is no part a wire consumes");
    let mech = p.mech.as_ref().unwrap();
    assert!(mech.front_is_round());
    assert_eq!(mech.holes[0].hole, Hole::Cutout(Cutout::D22));
    let enc = p.sections.enclosure.as_ref().unwrap();
    assert_eq!(
        (enc.doors[0].hinge.clone(), enc.doors[1].width),
        (Hinge::Left, None)
    );
    assert_eq!(enc.usable_depth, Some(Nm(200 * MM)));
    assert_eq!(
        p.sections.cable.as_ref().and_then(|c| c.outer_diameter),
        Some(Nm(10_500_000))
    );
    let gland = p.sections.gland.as_ref().unwrap();
    assert_eq!((gland.thread.clone(), gland.nut), (Gland::M20, Some(Nm(27 * MM))));
}

/// Round only where the width is the height.
#[test]
fn a_front_is_round_only_where_it_is_square() {
    let mut m = MechData {
        width: Some(Nm(30 * MM)),
        height: Some(Nm(30 * MM)),
        front_shape: Some(FrontShape::Round),
        ..MechData::default()
    };
    assert!(m.front_is_round());
    m.height = Some(Nm(40 * MM));
    assert!(!m.front_is_round());
    m.height = m.width;
    m.front_shape = Some(FrontShape::Rectangle);
    assert!(!m.front_is_round());
    m.front_shape = Some(FrontShape::Round);
    m.width = None;
    m.height = None;
    assert!(!m.front_is_round(), "no size, no circle");
}

/// A cable's chosen gland follows its accessory choices.
#[test]
fn a_cable_reads_its_chosen_gland() {
    let json = concat!(
        r#"{"id":"0190a000-0000-7000-8000-000000000001","tag":{"letter":"W","counter":1},"#,
        r#""accessory_choices":[{"of":"0190a000-0000-7000-8000-000000000003"}],"#,
        r#""gland":{"copy":"0190a000-0000-7000-8000-000000000005"},"function_text":"Supply"}"#
    );
    let d: Device = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&d).unwrap(), json);
    assert!(d.gland.is_some());
}

/// The workshop's hole settings follow the label profile; a self-tapping rule and the datum are written, the
/// default tapped rule is not.
#[test]
fn the_workshop_reads_its_hole_settings() {
    let json = concat!(
        r#"{"workshop":{"cut_increment":5000000,"fixing":"self_tapping","#,
        r#""rail_fixing":{"screw":"m5","pitch":300000000,"from_end":25000000},"#,
        r#""duct_fixing":{"screw":"m4","pitch":250000000,"from_end":20000000},"min_web":5000000,"datum":"top_left"}}"#
    );
    let s: ProjectSettings = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&s).unwrap(), json);
    assert_eq!(s.workshop.fixing, FixingRule::SelfTapping);
    assert_eq!(
        s.workshop.rail_fixing.as_ref().map(|f| f.screw.clone()),
        Some(Screw::M5)
    );
    assert_eq!(s.workshop.datum, Some(Corner::TopLeft));
    let mut tapped = s.clone();
    tapped.workshop.fixing = FixingRule::Tapped;
    assert!(!serde_json::to_string(&tapped).unwrap().contains("\"fixing\":"));
}

/// The one parser of a hole's text form, by kind.
#[test]
fn a_hole_parses_by_its_kind() {
    let mm = |n: i64| Nm(n * MM);
    for (kind, text, hole) in [
        (HoleKind::Cutout, "22 mm", Hole::Cutout(Cutout::D22)),
        (HoleKind::Cutout, "Ø 30", Hole::Cutout(Cutout::D30)),
        (HoleKind::Fixing, "M5", Hole::Fixing(Screw::M5)),
        (HoleKind::Gland, "m20", Hole::Gland(Gland::M20)),
        (HoleKind::Round, "Ø 4.5", Hole::Round { d: Nm(4_500_000) }),
        (HoleKind::Round, "4,5", Hole::Round { d: Nm(4_500_000) }),
        (HoleKind::Round, "6 mm", Hole::Round { d: mm(6) }),
        (
            HoleKind::Rect,
            "92 × 92 R 2",
            Hole::Rect {
                w: mm(92),
                h: mm(92),
                r: Some(mm(2)),
            },
        ),
        (
            HoleKind::Rect,
            "92x92",
            Hole::Rect {
                w: mm(92),
                h: mm(92),
                r: None,
            },
        ),
        (
            HoleKind::Rect,
            "92 x 45, R 2",
            Hole::Rect {
                w: mm(92),
                h: mm(45),
                r: Some(mm(2)),
            },
        ),
    ] {
        assert_eq!(Hole::parse(kind, text), Ok(hole.clone()), "{text}");
        assert_eq!(
            Hole::parse(kind, &hole.to_string()),
            Ok(hole),
            "the text form reads back"
        );
    }
    assert_eq!(
        Hole::parse(HoleKind::Fixing, "M7"),
        Err(HoleParseError::NotAScrew { text: "M7".into() })
    );
    assert_eq!(
        Hole::parse(HoleKind::Cutout, "25 mm"),
        Err(HoleParseError::NotACutout { text: "25 mm".into() })
    );
    assert_eq!(
        Hole::parse(HoleKind::Gland, "M22"),
        Err(HoleParseError::NotAGland { text: "M22".into() })
    );
    assert_eq!(
        Hole::parse(HoleKind::Round, "-3"),
        Err(HoleParseError::NotPositive)
    );
    assert_eq!(
        Hole::parse(HoleKind::Round, "0"),
        Err(HoleParseError::NotPositive)
    );
    assert_eq!(
        Hole::parse(HoleKind::Round, "wide"),
        Err(HoleParseError::NoNumber)
    );
    assert_eq!(
        Hole::parse(HoleKind::Rect, "92"),
        Err(HoleParseError::NotRect { text: "92".into() })
    );
    assert_eq!(
        Hole::parse(HoleKind::Rect, "0 × 92"),
        Err(HoleParseError::NotPositive)
    );
}

/// A project's own copy as an earlier build saved it, with `meta.source.fork_of` written by hand: `fork_of` reads its
/// library, item and version, and `set_fork_of` writes the same bytes again.
#[test]
fn a_fork_saved_before_reads_and_writes_its_fork_of_as_it_was() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_MECH","library_id":"@LIB","item_id":"@ITEM_PSU","version":"0.1.1","sha256":"@HASH","#,
        r#""meta":{"source":{"fork_of":{"item":"79825947-aa61-5360-8e00-c535c2db0168","#,
        r#""library":"75d07cef-7a38-5814-a2ca-4610c08d9a8b","version":"0.1.1"}}},"#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^0.1"},"part_number":"EW-GEN-FER-1.5"}"#
    ));
    let doc: LibDocument = serde_json::from_str(&line).unwrap();
    assert_eq!(serde_json::to_string(&doc).unwrap(), line, "not canonical");
    let LibDocument::Part(mut copy) = doc else {
        panic!("a part")
    };
    let fork = copy.item.meta.fork_of().expect("a fork");
    assert_eq!(
        (
            fork.library.to_string().as_str(),
            fork.item.to_string().as_str(),
            fork.version.as_str()
        ),
        (
            "75d07cef-7a38-5814-a2ca-4610c08d9a8b",
            "79825947-aa61-5360-8e00-c535c2db0168",
            "0.1.1"
        )
    );
    copy.item.meta.extra.clear();
    assert!(copy.item.meta.fork_of().is_none());
    copy.item.meta.set_fork_of(&fork);
    assert_eq!(serde_json::to_string(&LibDocument::Part(copy)).unwrap(), line);
}
