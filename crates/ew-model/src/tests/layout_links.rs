// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The links of the layout kinds (WP-S4.01 D4, LM-2, LM-6; doc 03 §4.2, §16): a placement cannot outlive its device
//! or function (cascade, EW-LAY-002), an occupied carrier and an enclosure's device cannot be removed (forbid), a
//! separate surface part's device leaves the surface without one (unlink), the containment chain cascades, and a
//! location with a layout space cannot be deleted. The three named indexes agree with a rebuild.

use ew_ids::{Id, Uuid};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Enclosure, Extra, Kind, LayoutSpace, MountKind,
    MountingSurface, PartRef, Placement, RailParams, Record, StructureNode, SurfaceKind,
};
use ew_units::{Nm, Pose3, Pt2};

use super::fixture::*;
use crate::{AnyKey, ApplyOptions, BaseIndexes, Entity, Model, ModelError, Op, Rev, invert_batch};

const LOC_B: Id<StructureNode> = Id::from_uuid(Uuid::from_u128(0x9500));
const SPACE: Id<LayoutSpace> = Id::from_uuid(Uuid::from_u128(0x9501));
const ENCL: Id<Enclosure> = Id::from_uuid(Uuid::from_u128(0x9502));
const SURF: Id<MountingSurface> = Id::from_uuid(Uuid::from_u128(0x9503));
const RAIL: Id<Carrier> = Id::from_uuid(Uuid::from_u128(0x9504));
const EMPTY_RAIL: Id<Carrier> = Id::from_uuid(Uuid::from_u128(0x9505));
const PL_F1: Id<Placement> = Id::from_uuid(Uuid::from_u128(0x9510));
const PL_G1: Id<Placement> = Id::from_uuid(Uuid::from_u128(0x9511));
const PL_AUX: Id<Placement> = Id::from_uuid(Uuid::from_u128(0x9512));

fn key<T: Entity>(id: crate::rev::RowId<T>) -> AnyKey {
    T::key(id)
}

fn rail(id: Id<Carrier>) -> Carrier {
    Carrier {
        id,
        v: 1,
        surface: SURF,
        kind: CarrierKind::Rail,
        part: Some(PartRef::new(PART_MCB)),
        path: vec![Pt2::new(Nm(0), Nm(0)), Pt2::new(Nm(500_000_000), Nm(0))],
        params: CarrierParams::Rail(RailParams {
            profile: MountKind::Ts35x7_5,
            extra: Extra::new(),
        }),
        pe_bond: false,
        extra: Extra::new(),
    }
}

fn placement(id: Id<Placement>, device: Id<ew_schema::Device>, carrier: CarrierRef) -> Placement {
    Placement {
        id,
        v: 1,
        device,
        function: None,
        part: None,
        carrier,
        pose: Pose3::IDENTITY,
        locked: false,
        extra: Extra::new(),
    }
}

/// The mini golden project with a layout space on the location node `LOC_B`, whose enclosure is -102F2 and whose
/// plate surface is the device -3W1; a rail holding -101F1, an empty rail, -102G1 on the surface itself and the
/// auxiliary function of -102F2 placed on its own.
fn layout_records() -> Vec<Record> {
    vec![
        Record::StructureNode(structure_node(LOC_B, Aspect::Location, "J-E-206", None)),
        Record::LayoutSpace(LayoutSpace {
            id: SPACE,
            v: 1,
            location: LOC_B,
            name: None,
            extra: Extra::new(),
        }),
        Record::Enclosure(Enclosure {
            id: ENCL,
            v: 1,
            space: SPACE,
            device: DEV_102F2,
            pose: Pose3::IDENTITY,
            extra: Extra::new(),
        }),
        Record::MountingSurface(MountingSurface {
            id: SURF,
            v: 1,
            enclosure: ENCL,
            kind: SurfaceKind::Plate,
            device: Some(DEV_3W1),
            frame: Pose3::IDENTITY,
            usable: vec![],
            pe_bond: false,
            extra: Extra::new(),
        }),
        Record::Carrier(rail(RAIL)),
        Record::Carrier(rail(EMPTY_RAIL)),
        Record::Placement(placement(PL_F1, DEV_101F1, CarrierRef::Carrier(RAIL))),
        Record::Placement(placement(PL_G1, DEV_102G1, CarrierRef::Surface(SURF))),
        Record::Placement(Placement {
            function: Some(FN_102F2_AUX),
            ..placement(PL_AUX, DEV_X1031, CarrierRef::Carrier(RAIL))
        }),
    ]
}

fn with_layout() -> Result<Model, ModelError> {
    let (project, mut records, lib) = mini_golden_parts();
    records.extend(layout_records());
    Model::from_records(project, records, lib)
}

fn referenced(r: Result<Vec<Op>, ModelError>) -> (String, AnyKey) {
    match r {
        Err(ModelError::Referenced { link, referrer, .. }) => (link, referrer),
        other => panic!("expected Referenced, got {other:?}"),
    }
}

fn removes(ops: &[Op], k: AnyKey) -> bool {
    ops.iter().any(|op| op.key() == k && op.after().is_none())
}

/// The layout records load into their tables; the named indexes list placements by device and by carrier and
/// carriers by surface, and a placement on the surface itself is reverse-mapped by the generic referrer index.
#[test]
fn layout_records_load_into_their_tables_and_indexes() {
    let m = with_layout().expect("valid");
    assert_eq!(m.table::<Placement>().len(), 3);
    let idx = m.indexes();
    assert_eq!(
        idx.placements_by_device()
            .get(&DEV_101F1)
            .copied()
            .collect::<Vec<_>>(),
        [PL_F1]
    );
    assert_eq!(
        idx.placements_by_carrier()
            .get(&RAIL)
            .copied()
            .collect::<Vec<_>>(),
        [PL_F1, PL_AUX]
    );
    assert_eq!(idx.placements_by_carrier().get(&EMPTY_RAIL).count(), 0);
    assert_eq!(
        idx.carriers_by_surface().get(&SURF).copied().collect::<Vec<_>>(),
        [RAIL, EMPTY_RAIL]
    );
    let on_surface: Vec<AnyKey> = m
        .referrers(&key::<MountingSurface>(SURF))
        .into_iter()
        .filter(|(l, _)| l.path() == "placement.carrier.surface")
        .map(|(_, k)| k)
        .collect();
    assert_eq!(on_surface, [key::<Placement>(PL_G1)]);
    assert!(BaseIndexes::rebuild(&m).same_entries(m.indexes()));
}

/// LM-6: deleting a placed device removes its placement in the same plan; the undo restores it.
#[test]
fn deleting_a_placed_device_removes_its_placement() {
    let m = with_layout().expect("valid");
    let ops = m.delete_plan(&key::<ew_schema::Device>(DEV_101F1)).unwrap();
    assert!(removes(&ops, key::<Placement>(PL_F1)), "{ops:?}");
    let after = m
        .apply_with(&ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
    assert!(after.table::<Placement>().get(&PL_F1).is_none());
    assert_eq!(
        after
            .indexes()
            .placements_by_carrier()
            .get(&RAIL)
            .copied()
            .collect::<Vec<_>>(),
        [PL_AUX]
    );
    let undone = after
        .apply_history(&invert_batch(&ops), Rev(2))
        .expect("the undo applies");
    assert!(undone.content_eq(&m));
}

/// A function placed on its own leaves with its function (cascade), and the device's other placement stays.
#[test]
fn deleting_a_placed_function_removes_its_placement() {
    let m = with_layout().expect("valid");
    let ops = m.delete_plan(&key::<ew_schema::Function>(FN_102F2_AUX)).unwrap();
    assert!(removes(&ops, key::<Placement>(PL_AUX)), "{ops:?}");
    assert!(!removes(&ops, key::<Placement>(PL_F1)));
}

/// LM-6: an occupied carrier cannot be deleted other than by `layout.carrier.delete` (forbid); an empty one can.
#[test]
fn an_occupied_carrier_is_referenced_and_an_empty_one_goes() {
    let m = with_layout().expect("valid");
    let (link, referrer) = referenced(m.delete_plan(&key::<Carrier>(RAIL)));
    assert_eq!(link, "placement.carrier.carrier");
    assert!([key::<Placement>(PL_F1), key::<Placement>(PL_AUX)].contains(&referrer));
    let ops = m.delete_plan(&key::<Carrier>(EMPTY_RAIL)).unwrap();
    assert_eq!(ops.len(), 1);
    // The surface a placement sits on is kept the same way.
    let (link, _) = referenced(m.delete_plan(&key::<MountingSurface>(SURF)));
    assert!(link.starts_with("placement.carrier."), "{link}");
}

/// LM-6: an enclosure's device cannot be deleted under it (forbid); a surface part's device can, and the surface
/// stays without one (unlink).
#[test]
fn an_enclosure_keeps_its_device_and_a_surface_lets_its_device_go() {
    let m = with_layout().expect("valid");
    let (link, referrer) = referenced(m.delete_plan(&key::<ew_schema::Device>(DEV_102F2)));
    assert_eq!(
        (link.as_str(), referrer),
        ("enclosure.device", key::<Enclosure>(ENCL))
    );
    let ops = m.delete_plan(&key::<ew_schema::Device>(DEV_3W1)).unwrap();
    let op = ops
        .iter()
        .find(|op| op.key() == key::<MountingSurface>(SURF))
        .expect("the surface is in the plan");
    let after_row = op.after().expect("modified, not removed");
    assert_eq!(after_row.get::<MountingSurface>().unwrap().device, None);
    plan_commits(&m, &ops);
}

/// LM-2: the location of a layout space cannot be deleted while the space exists (forbid); the space with an
/// occupied carrier cannot be deleted either (the cascade reaches the forbid), and once the placements are gone it
/// takes its enclosure, surface and carriers with it.
#[test]
fn a_space_keeps_its_location_and_cascades_once_empty() {
    let m = with_layout().expect("valid");
    let (link, referrer) = referenced(m.delete_plan(&key::<StructureNode>(LOC_B)));
    assert_eq!(
        (link.as_str(), referrer),
        ("layout_space.location", key::<LayoutSpace>(SPACE))
    );
    let (link, _) = referenced(m.delete_plan(&key::<LayoutSpace>(SPACE)));
    assert!(link.starts_with("placement.carrier."), "{link}");

    let placements = [PL_F1, PL_G1, PL_AUX].map(key::<Placement>);
    let cleared = m.delete_plan_many(&placements).unwrap();
    let empty = m
        .apply_with(&cleared, Rev(1), &ApplyOptions::default())
        .expect("the placements go");
    let ops = empty.delete_plan(&key::<LayoutSpace>(SPACE)).unwrap();
    for k in [
        key::<LayoutSpace>(SPACE),
        key::<Enclosure>(ENCL),
        key::<MountingSurface>(SURF),
        key::<Carrier>(RAIL),
        key::<Carrier>(EMPTY_RAIL),
    ] {
        assert!(removes(&ops, k), "{k:?} in {ops:?}");
    }
    let after = empty
        .apply_with(&ops, Rev(2), &ApplyOptions::default())
        .expect("the cascade commits");
    assert!(after.table::<LayoutSpace>().is_empty());
    assert!(BaseIndexes::rebuild(&after).same_entries(after.indexes()));
}

/// EW-INV-001 holds for the layout links: a placement naming no device does not load, and a batch that removes a
/// placed device without its placement is rejected.
#[test]
fn a_placement_without_its_device_is_rejected() {
    let (project, mut records, lib) = mini_golden_parts();
    records.extend(layout_records());
    records.push(Record::Placement(placement(
        Id::from_uuid(Uuid::from_u128(0x9520)),
        Id::from_uuid(Uuid::from_u128(0x9521)),
        CarrierRef::Carrier(RAIL),
    )));
    match Model::from_records(project, records, lib) {
        Err(ModelError::Invariant { id, .. }) => assert_eq!(id, "EW-INV-001"),
        other => panic!("expected EW-INV-001, got {other:?}"),
    }

    let m = with_layout().expect("valid");
    let plan = m.delete_plan(&key::<ew_schema::Device>(DEV_101F1)).unwrap();
    let without_placement: Vec<Op> = plan
        .into_iter()
        .filter(|op| op.key().0 != Kind::Placement)
        .collect();
    assert!(
        m.apply_with(&without_placement, Rev(1), &ApplyOptions::default())
            .is_err()
    );
}

fn plan_commits(m: &Model, ops: &[Op]) {
    let after = m
        .apply_with(ops, Rev(1), &ApplyOptions::default())
        .expect("the plan commits");
    assert!(BaseIndexes::rebuild(&after).same_entries(after.indexes()));
}
