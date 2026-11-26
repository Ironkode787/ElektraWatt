// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout geometry and the compatibility table of doc 12 §3.3.

use ew_schema::{CarrierKind, MechData, MountKind};
use ew_units::{Nm, Pose3, Pt2, Quat9};

use super::*;

const MM: i64 = 1_000_000;

fn p(u: i64, v: i64) -> Pt2 {
    Pt2::new(Nm(u * MM), Nm(v * MM))
}

fn pose(t: [i64; 3], q: Quarter) -> Pose3 {
    Pose3 {
        t: t.map(Nm),
        q: q.quat(),
    }
}

/// A rail is two points on one horizontal or vertical line; a duct an orthogonal polyline; anything else is
/// refused at the first bad segment.
#[test]
fn carrier_paths_are_axis_parallel() {
    assert_eq!(check_path(&CarrierKind::Rail, &[p(0, 100), p(500, 100)]), Ok(()));
    assert_eq!(check_path(&CarrierKind::Rail, &[p(0, 100), p(0, 600)]), Ok(()));
    assert_eq!(
        check_path(&CarrierKind::Rail, &[p(0, 0), p(10, 10)]),
        Err(PathError::Diagonal(0))
    );
    assert_eq!(
        check_path(&CarrierKind::Rail, &[p(0, 0), p(10, 0), p(10, 10)]),
        Err(PathError::RailPoints(3))
    );
    assert_eq!(
        check_path(&CarrierKind::Rail, &[p(5, 5), p(5, 5)]),
        Err(PathError::ZeroLength(0))
    );
    assert_eq!(
        check_path(&CarrierKind::Duct, &[p(0, 0), p(600, 0), p(600, 800)]),
        Ok(())
    );
    assert_eq!(
        check_path(&CarrierKind::Duct, &[p(0, 0), p(600, 0), p(650, 800)]),
        Err(PathError::Diagonal(1))
    );
    assert_eq!(
        check_path(&CarrierKind::Duct, &[p(0, 0)]),
        Err(PathError::DuctPoints(1))
    );
    assert_eq!(
        check_path(
            &CarrierKind::Unknown(serde_json::json!("busbar")),
            &[p(0, 0), p(3, 4)]
        ),
        Ok(())
    );
    assert_eq!(
        path_length(&[p(0, 0), p(600, 0), p(600, 800)]),
        i128::from(1400 * MM)
    );
}

/// A rail's axis: its start, its direction as quarter turns from `+u`, its length and the point at `s`.
#[test]
fn a_rail_axis_runs_in_one_of_four_directions() {
    let right = RailAxis::of(&[p(50, 100), p(550, 100)]).unwrap();
    assert_eq!((right.dir, right.length), (Quarter::new(0), i128::from(500 * MM)));
    assert_eq!(
        right.point_at(i128::from(96 * MM)),
        (i128::from(146 * MM), i128::from(100 * MM))
    );
    let down = RailAxis::of(&[p(10, 900), p(10, 100)]).unwrap();
    assert_eq!((down.dir, down.length), (Quarter::new(3), i128::from(800 * MM)));
    assert_eq!(
        down.point_at(i128::from(100 * MM)),
        (i128::from(10 * MM), i128::from(800 * MM))
    );
    assert_eq!(RailAxis::of(&[p(0, 0), p(1, 1)]), None);
    assert_eq!(RailAxis::of(&[p(0, 0)]), None);
}

/// The table of doc 12 §3.3: TS 35 devices fit TS 35 × 7.5 and TS 35 × 15, TS 32 only TS 32, TS 15 only TS 15; a
/// screw-mounted part fits no rail but a surface; no mount kind (or a newer one) cannot be judged.
#[test]
fn the_compatibility_table_matches_doc_12() {
    use MountKind::*;
    let fits = |m: MountKind, r: MountKind| fits_rail(Some(&m), &r);
    assert_eq!(fits(Ts35x7_5, Ts35x15), Fit::Fits);
    assert_eq!(fits(Ts35x15, Ts35x7_5), Fit::Fits);
    assert_eq!(
        fits(Ts35x7_5, Ts32),
        Fit::DoesNotFit,
        "-102F1 (TS 35) does not fit a TS 32 rail"
    );
    assert_eq!(fits(Ts32, Ts32), Fit::Fits);
    assert_eq!(fits(Ts32, Ts35x7_5), Fit::DoesNotFit);
    assert_eq!(fits(Ts15, Ts15), Fit::Fits);
    assert_eq!(fits(Ts15, Ts35x15), Fit::DoesNotFit);
    for kind in [Screw, BusbarAdapter, DoorCutout, OnParent, None] {
        assert_eq!(fits(kind.clone(), Ts35x7_5), Fit::DoesNotFit, "{kind:?}");
    }
    assert_eq!(fits_rail(Option::None, &Ts35x7_5), Fit::Unknown);
    assert_eq!(fits(Unknown(serde_json::json!("ts99")), Ts35x7_5), Fit::Unknown);
    assert_eq!(fits_surface(Some(&Screw)), Fit::Fits);
    assert_eq!(fits_surface(Some(&Ts35x7_5)), Fit::DoesNotFit);
    assert_eq!(fits_surface(Option::None), Fit::Unknown);
    for (device, rails) in RAIL_FITS {
        assert!(
            rails.contains(&device),
            "a profile fits its own rail: {}",
            device.name()
        );
    }
}

/// Quarter turns: the four canonical quaternions, their composition, their action on a vector, and a stored
/// quaternion read back (one unit off in the last place still counts; any tilt does not).
#[test]
fn quarter_turns_about_w() {
    assert_eq!(Quarter::new(0).quat(), Quat9::IDENTITY);
    assert_eq!(
        Quarter::new(1).quat().components(),
        [707_106_781, 0, 0, 707_106_781]
    );
    assert_eq!(Quarter::new(2).quat().components(), [0, 0, 0, 1_000_000_000]);
    assert_eq!(
        Quarter::new(3).quat().components(),
        [707_106_781, 0, 0, -707_106_781]
    );
    for k in 0..4 {
        assert_eq!(Quarter::of(Quarter::new(k).quat()), Some(Quarter::new(k)));
    }
    let near = Quat9::new(707_106_782, 0, 0, 707_106_781).unwrap();
    assert_eq!(Quarter::of(near), Some(Quarter::new(1)));
    let tilted = Quat9::new(707_106_781, 707_106_781, 0, 0).unwrap();
    assert_eq!(Quarter::of(tilted), None);
    assert_eq!(Quarter::new(3).then(Quarter::new(2)), Quarter::new(1));
    assert_eq!(Quarter::new(1).rotate((2, 1)), (-1, 2));
    assert_eq!(Quarter::new(2).rotate((2, 1)), (-2, -1));
    assert_eq!(Quarter::new(3).rotate((2, 1)), (1, -2));
}

/// Doc 12 §2.3: on a rail `t = (s, 0, 0)`, `s ∈ [0, L]`, a turn of 0° or 180°; on a surface `t = (u, v, 0)` and a turn
/// of `k · 90°`; a duct carries nothing.
#[test]
fn poses_keep_to_their_carriers_degrees_of_freedom() {
    let rail = PoseOn::Rail(RailAxis::of(&[p(0, 100), p(500, 100)]).unwrap());
    let l = 500 * MM;
    assert_eq!(check_pose(rail, &pose([0, 0, 0], Quarter::NONE)), Ok(()));
    assert_eq!(check_pose(rail, &pose([l, 0, 0], Quarter::HALF)), Ok(()));
    assert_eq!(
        check_pose(rail, &pose([l + 1, 0, 0], Quarter::NONE)),
        Err(PoseError::OffRail(i128::from(l + 1), i128::from(l)))
    );
    assert_eq!(
        check_pose(rail, &pose([-1, 0, 0], Quarter::NONE)),
        Err(PoseError::OffRail(-1, i128::from(l)))
    );
    assert_eq!(
        check_pose(rail, &pose([5, 1, 0], Quarter::NONE)),
        Err(PoseError::OffRailAxis)
    );
    assert_eq!(
        check_pose(rail, &pose([5, 0, 1], Quarter::NONE)),
        Err(PoseError::OffRailAxis)
    );
    assert_eq!(
        check_pose(rail, &pose([5, 0, 0], Quarter::new(1))),
        Err(PoseError::RailTurn)
    );
    for k in 0..4 {
        assert_eq!(
            check_pose(PoseOn::Surface, &pose([-5, 7, 0], Quarter::new(k))),
            Ok(())
        );
    }
    assert_eq!(
        check_pose(PoseOn::Surface, &pose([0, 0, 3], Quarter::NONE)),
        Err(PoseError::OffSurface)
    );
    let tilted = Pose3 {
        t: [Nm(0); 3],
        q: Quat9::new(707_106_781, 707_106_781, 0, 0).unwrap(),
    };
    assert_eq!(check_pose(PoseOn::Surface, &tilted), Err(PoseError::SurfaceTurn));
    assert_eq!(check_pose(PoseOn::Duct, &Pose3::IDENTITY), Err(PoseError::OnDuct));
    assert_eq!(
        PoseError::OffRail(i128::from(512_500_000), i128::from(l)).to_string(),
        "the pose is off the rail: s = 512.5 mm is outside [0, 500 mm]"
    );
}

/// `rehome_to_surface` (doc 12 §3.6): the part keeps its place and turn. A 18 × 90 mm part with its clip point at the
/// centre, at s = 96 mm on a rail from (50, 100) to the right, has its origin at (137, 55); flipped 180°, at (155, 145);
/// on a rail running down from (20, 900), turned by 270°.
#[test]
fn a_rail_pose_rehomes_to_the_equivalent_surface_pose() {
    let mech = MechData {
        width: Some(Nm(18 * MM)),
        height: Some(Nm(90 * MM)),
        ..MechData::default()
    };
    let clip = clip_offset(Some(&mech));
    assert_eq!(clip, (i128::from(9 * MM), i128::from(45 * MM)));
    let right = RailAxis::of(&[p(50, 100), p(550, 100)]).unwrap();
    assert_eq!(
        rehome_pose(&right, &pose([96 * MM, 0, 0], Quarter::NONE), clip),
        Some(pose([137 * MM, 55 * MM, 0], Quarter::NONE))
    );
    assert_eq!(
        rehome_pose(&right, &pose([96 * MM, 0, 0], Quarter::HALF), clip),
        Some(pose([155 * MM, 145 * MM, 0], Quarter::HALF))
    );
    let down = RailAxis::of(&[p(20, 900), p(20, 100)]).unwrap();
    // clip point (20, 800); turned 270°: the clip offset (9, 45) becomes (45, -9)
    assert_eq!(
        rehome_pose(&down, &pose([100 * MM, 0, 0], Quarter::NONE), clip),
        Some(pose([-25 * MM, 809 * MM, 0], Quarter::new(3)))
    );
    assert_eq!(rehome_pose(&right, &pose([0, 1, 0], Quarter::NONE), clip), None);
    // the clip point: rail_offset first, else the centre, else the pose itself
    let offset = MechData {
        rail_offset: Some(p(9, 30)),
        ..mech.clone()
    };
    assert_eq!(
        clip_offset(Some(&offset)),
        (i128::from(9 * MM), i128::from(30 * MM))
    );
    assert_eq!(clip_offset(Some(&MechData::default())), (0, 0));
    assert_eq!(clip_offset(None), (0, 0));
}

/// The numbers: a 650 × 900 mm plate in a 750 × 1000 mm enclosure sits at t = (50, 50, 0) mm with the usable
/// rectangle (0, 0)–(650, 900) mm; unknown sizes give the origin and, without a plate size, no usable polygon.
#[test]
fn a_plate_is_centred_in_its_enclosure() {
    let mm = |v: i64| Some(Nm(v * MM));
    let (frame, usable) = plate_frame((mm(750), mm(1000)), (mm(650), mm(900)));
    assert_eq!(frame, pose([50 * MM, 50 * MM, 0], Quarter::NONE));
    assert_eq!(usable, vec![p(0, 0), p(650, 0), p(650, 900), p(0, 900)]);
    let (frame, usable) = plate_frame((None, mm(1000)), (mm(650), mm(900)));
    assert_eq!((frame, usable.len()), (Pose3::IDENTITY, 4));
    let (frame, usable) = plate_frame((mm(750), mm(1000)), (mm(650), None));
    assert_eq!((frame, usable), (Pose3::IDENTITY, vec![]));
}

/// Millimetre text for messages is exact and trims fraction zeros.
#[test]
fn millimetres_are_printed_exactly() {
    use crate::text::mm_exact;
    assert_eq!(mm_exact(96_000_000), "96");
    assert_eq!(mm_exact(7_500_000), "7.5");
    assert_eq!(mm_exact(-1), "-0.000001");
    assert_eq!(mm_exact(0), "0");
}

/// An enclosure section of `width × height × depth` mm with `doors`.
fn section(w: i64, h: i64, d: i64, doors: Vec<ew_schema::DoorSpec>) -> ew_schema::EnclosureSection {
    ew_schema::EnclosureSection {
        width: Some(Nm(w * MM)),
        height: Some(Nm(h * MM)),
        depth: Some(Nm(d * MM)),
        doors,
        edge_margin: Some(Nm(30 * MM)),
        ..Default::default()
    }
}

fn door(hinge: ew_schema::Hinge, width: Option<i64>) -> ew_schema::DoorSpec {
    ew_schema::DoorSpec {
        hinge,
        width: width.map(|w| Nm(w * MM)),
        extra: Default::default(),
    }
}

/// One door covers the whole front, hinged on the left; of two, each its width at its place; a side, the top and the
/// bottom are the depth by the height or the width; without a depth they are not offered.
#[test]
fn a_cabinet_s_faces_take_their_sizes_and_places_from_the_part() {
    use ew_schema::{Hinge, SurfaceKind as K};
    let one = section(600, 800, 250, Vec::new());
    assert_eq!(door_count(&one), 1);
    assert_eq!(
        face_size(&one, &K::Door, Some(0)),
        Some((Nm(600 * MM), Nm(800 * MM)))
    );
    assert_eq!(
        face_size(&one, &K::Left, None),
        Some((Nm(250 * MM), Nm(800 * MM)))
    );
    assert_eq!(
        face_size(&one, &K::Bottom, None),
        Some((Nm(600 * MM), Nm(250 * MM)))
    );
    let (frame, sheet) = surface_frame(&one, &K::Door, Some(0), None).unwrap();
    assert_eq!(
        frame.t,
        [Nm(0), Nm(0), Nm(250 * MM)],
        "the door stands at the front"
    );
    assert_eq!(frame.q, Quat9::IDENTITY, "seen from outside: nothing is mirrored");
    assert_eq!(sheet[2], p(600, 800));
    // A bottom is turned a quarter about x, a right side a quarter about y at the right front edge.
    let (bottom, _) = surface_frame(&one, &K::Bottom, None, None).unwrap();
    assert_ne!(bottom.q, Quat9::IDENTITY);
    let (right, _) = surface_frame(&one, &K::Right, None, None).unwrap();
    assert_eq!(right.t, [Nm(600 * MM), Nm(0), Nm(250 * MM)]);
    let two = section(
        1200,
        2000,
        400,
        vec![door(Hinge::Left, Some(500)), door(Hinge::Right, None)],
    );
    assert_eq!(
        face_size(&two, &K::Door, Some(1)),
        Some((Nm(700 * MM), Nm(2000 * MM)))
    );
    let (second, _) = surface_frame(&two, &K::Door, Some(1), None).unwrap();
    assert_eq!(
        second.t[0],
        Nm(500 * MM),
        "the right door starts where the left one ends"
    );
    assert_eq!(
        face_size(&two, &K::Door, Some(2)),
        None,
        "a door the part does not list"
    );
    let flat = ew_schema::EnclosureSection {
        depth: None,
        ..one.clone()
    };
    assert_eq!(face_size(&flat, &K::Top, None), None);
    assert_eq!(surface_frame(&flat, &K::Top, None, None), None);
    assert!(
        surface_frame(&flat, &K::Top, None, Some((Nm(600 * MM), Nm(200 * MM)))).is_some(),
        "a size typed"
    );
    assert_eq!(
        surface_frame(&one, &K::Plate, None, None),
        None,
        "the plate is plate_frame's"
    );
}

/// A door's words follow how many doors the cabinet has.
#[test]
fn a_door_is_named_by_its_place_among_the_doors() {
    use crate::text::surface_word;
    use ew_schema::SurfaceKind as K;
    assert_eq!(surface_word(&K::Door, Some(0), 1).to_string(), "Door");
    assert_eq!(surface_word(&K::Door, Some(0), 2).to_string(), "Left door");
    assert_eq!(surface_word(&K::Door, Some(1), 2).to_string(), "Right door");
    assert_eq!(surface_word(&K::Door, Some(2), 3).to_string(), "Door 3");
    assert_eq!(
        crate::text::surface_caption(&K::Bottom, None, 1).to_string(),
        "Bottom, seen from below; the door's edge at the top"
    );
}
