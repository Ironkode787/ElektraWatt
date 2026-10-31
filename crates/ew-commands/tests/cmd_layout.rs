// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.01 part 1 at the command layer: the `layout.*` commands of D6 (LM-2 to LM-6) and the part follow of D7 in
//! `part.assign` (LM-7), each refusal with its text or pointer, each command one undo step.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddCarrier, AssignModeParam, AssignPart, CarrierKindParam, CarrierParamsParam, CarrierThen,
    CreateEnclosure, CreateLayoutSpace, DeleteCarrier, DeleteEnclosure, DeleteLayoutSpace, DeleteSelection,
    EditCarrier, EditPartCopy, KindName, LockPlacement, MovePlacement, PlaceDevice, PlaceOnParam,
    RemovePlacement, TargetRef,
};
use ew_commands::{CmdError, Command, Engine, Origin};
use ew_ids::{Id, v5};
use ew_model::{Model, ModelError};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierRef, Device, Enclosure, EnclosureSection, Kind, LayoutSpace, LibCopy, LibDocument,
    MechData, MountKind, MountingSurface, Part, PartRef, PartUse, Placement, PlateSection, RailSection,
    Record, StructureNode, SurfaceKind,
};
use ew_units::{Dec3, Nm, Pose3, Pt2, Quat9};

const MM: i64 = 1_000_000;

/// The library of this file's part copies.
const LIB_L: u128 = 0xd000;
const ITEM_ENCL: u128 = 0xd001;
const ITEM_PLATE: u128 = 0xd002;
const ITEM_RAIL35: u128 = 0xd003;
const ITEM_RAIL32: u128 = 0xd004;
const ITEM_MCB: u128 = 0xd005;
const ITEM_MCB_WIDE: u128 = 0xd006;
const ITEM_ACC: u128 = 0xd007;
const ITEM_G_RAIL: u128 = 0xd008;

/// The enclosure device (-UC1), its plate child, two MCBs, a G-rail device, a second enclosure device (-UC20, no
/// plate child) and an empty location.
const DEV_UC1: Id<Device> = id(0xa00);
const DEV_PLATE: Id<Device> = id(0xa01);
const DEV_K1: Id<Device> = id(0xa02);
const DEV_K2: Id<Device> = id(0xa03);
const DEV_K3: Id<Device> = id(0xa04);
const DEV_UC2: Id<Device> = id(0xa05);
const LOC_206: Id<StructureNode> = id(0xa10);

fn copy_id(item: u128) -> Id<Part> {
    v5::lib_copy_id(uuid(LIB_L), uuid(item))
}

fn copy(item: u128, number: &str) -> LibCopy<Part> {
    let mut c = part(copy_id(item), number);
    c.library_id = Id::from_uuid(uuid(LIB_L));
    c.item_id = Id::from_uuid(uuid(item));
    c
}

fn mech(kind: MountKind, w: i64, h: i64) -> MechData {
    MechData {
        width: Some(Nm(w * MM)),
        height: Some(Nm(h * MM)),
        mount_kind: Some(kind),
        ..MechData::default()
    }
}

fn uses(parts: &[u128]) -> Vec<PartUse> {
    parts
        .iter()
        .map(|i| PartUse {
            part: PartRef::new(copy_id(*i)),
            qty: Dec3::ONE,
            extra: Default::default(),
        })
        .collect()
}

/// The part copies: an enclosure (750 × 1000 × 420 mm, its plate 650 × 900 mm), TS 35 and TS 32 rails, a TS 35 MCB
/// (18 × 90 mm), a wider TS 35 MCB, an accessory mounted on its parent and a TS 32 device.
fn copies() -> Vec<LibCopy<Part>> {
    let mut encl = copy(ITEM_ENCL, "NSYS3D10840P");
    encl.item.sections.enclosure = Some(EnclosureSection {
        width: Some(Nm(750 * MM)),
        height: Some(Nm(1000 * MM)),
        depth: Some(Nm(420 * MM)),
        plate: Some(lib_ref_to(uuid(LIB_L), uuid(ITEM_PLATE))),
        ..EnclosureSection::default()
    });
    let mut plate = copy(ITEM_PLATE, "NSYMM108");
    plate.item.sections.mounting_plate = Some(PlateSection {
        width: Some(Nm(650 * MM)),
        height: Some(Nm(900 * MM)),
        ..PlateSection::default()
    });
    let rail = |item, profile| {
        let mut c = copy(item, "RAIL");
        c.item.sections.rail = Some(RailSection {
            profile,
            stock_length: None,
            extra: Default::default(),
        });
        c
    };
    let device = |item, m: MechData| {
        let mut c = copy(item, "DEV");
        c.item.mech = Some(m);
        c
    };
    vec![
        encl,
        plate,
        rail(ITEM_RAIL35, MountKind::Ts35x7_5),
        rail(ITEM_RAIL32, MountKind::Ts32),
        device(ITEM_MCB, mech(MountKind::Ts35x7_5, 18, 90)),
        device(ITEM_MCB_WIDE, mech(MountKind::Ts35x7_5, 36, 90)),
        device(ITEM_ACC, mech(MountKind::OnParent, 9, 45)),
        device(ITEM_G_RAIL, mech(MountKind::Ts32, 12, 60)),
    ]
}

/// The mini golden project with the devices and copies above.
fn model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut uc1 = device(DEV_UC1, "UC", 1);
    uc1.parts = uses(&[ITEM_ENCL]);
    let mut plate = device(DEV_PLATE, "UC", 2);
    plate.parent = Some(DEV_UC1);
    plate.parts = uses(&[ITEM_PLATE]);
    let mut k1 = device(DEV_K1, "K", 1);
    k1.parts = uses(&[ITEM_MCB]);
    let mut k2 = device(DEV_K2, "K", 2);
    k2.parts = uses(&[ITEM_ACC, ITEM_MCB_WIDE, ITEM_MCB]);
    let mut k3 = device(DEV_K3, "K", 3);
    k3.parts = uses(&[ITEM_G_RAIL]);
    let mut uc2 = device(DEV_UC2, "UC", 20);
    uc2.parts = uses(&[ITEM_ENCL]);
    records.extend([uc1, plate, k1, k2, k3, uc2].map(Record::Device));
    records.push(Record::StructureNode(structure_node(
        LOC_206,
        Aspect::Location,
        "J-E-206",
        None,
    )));
    let mut lib = library();
    lib.extend(copies().into_iter().map(LibDocument::Part));
    Model::from_records(p, records, lib).expect("valid")
}

fn p(u: i64, v: i64) -> Pt2 {
    Pt2::new(Nm(u * MM), Nm(v * MM))
}

fn at(s: i64) -> Pose3 {
    Pose3 {
        t: [Nm(s * MM), Nm(0), Nm(0)],
        q: Quat9::IDENTITY,
    }
}

fn rejected(r: Result<ew_commands::Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn pointer(r: Result<ew_commands::Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::InvalidParams { path, .. }) => path,
        other => panic!("expected invalid parameters, got {other:?}"),
    }
}

fn referenced(r: Result<ew_commands::Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Model(ModelError::Referenced { link, .. })) => link,
        other => panic!("expected a forbid refusal, got {other:?}"),
    }
}

fn created<T>(r: ew_commands::Receipt) -> Id<T> {
    r.outcome.primary.expect("a primary record").1.cast()
}

fn rows<T: ew_model::Entity + Clone>(e: &Engine) -> Vec<T> {
    e.model()
        .table::<T>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .collect()
}

fn get<T: ew_model::Entity<Marker = T> + Clone>(e: &Engine, id: Id<T>) -> Option<T> {
    e.model().table::<T>().get(&id).map(|r| (**r).clone())
}

/// A layout: the space of +J-E-2416, -UC1's enclosure with its plate, a TS 35 rail (20, 800)–(620, 800) mm with its
/// part and a TS 32 rail (20, 600)–(620, 600) mm.
struct Layout {
    e: Engine,
    space: Id<LayoutSpace>,
    enclosure: Id<Enclosure>,
    surface: Id<MountingSurface>,
    rail: Id<Carrier>,
    rail32: Id<Carrier>,
}

fn rail_cmd(surface: Id<MountingSurface>, y: i64, item: u128, profile: MountKind) -> AddCarrier {
    AddCarrier {
        surface,
        kind: CarrierKindParam::Rail,
        part: Some(copy_id(item)),
        path: vec![p(20, y), p(620, y)],
        params: CarrierParamsParam::Rail { profile },
        pe_bond: Some(true),
    }
}

fn layout() -> Layout {
    let mut e = engine(model());
    let space = created(
        e.execute(&CreateLayoutSpace {
            location: LOC,
            name: None,
        })
        .unwrap(),
    );
    let enclosure = created(
        e.execute(&CreateEnclosure {
            space,
            device: DEV_UC1,
            pose: None,
            pe_bond: Some(true),
        })
        .unwrap(),
    );
    let surface = rows::<MountingSurface>(&e)[0].id;
    let rail = created(
        e.execute(&rail_cmd(surface, 800, ITEM_RAIL35, MountKind::Ts35x7_5))
            .unwrap(),
    );
    let rail32 = created(
        e.execute(&rail_cmd(surface, 600, ITEM_RAIL32, MountKind::Ts32))
            .unwrap(),
    );
    Layout {
        e,
        space,
        enclosure,
        surface,
        rail,
        rail32,
    }
}

fn place(device: Id<Device>, on: PlaceOnParam, pose: Pose3) -> PlaceDevice {
    PlaceDevice {
        device,
        part: None,
        function: None,
        carrier: on,
        pose,
    }
}

/// LM-2: one layout space per location, the second refused naming the first; a node that is not a location refused;
/// the location's structure node cannot be deleted while the space exists, and can once it is gone.
#[test]
fn lm2_a_location_has_one_layout_space() {
    let mut e = engine(model());
    let r = e
        .execute(&CreateLayoutSpace {
            location: LOC_206,
            name: Some(ident("Cabinet 206")),
        })
        .unwrap();
    let space: Id<LayoutSpace> = created(r);
    let spaces = rows::<LayoutSpace>(&e);
    assert_eq!(spaces.len(), 1);
    assert_eq!(
        (spaces[0].location, spaces[0].name.clone()),
        (LOC_206, Some(ident("Cabinet 206")))
    );
    let second = rejected(e.execute(&CreateLayoutSpace {
        location: LOC_206,
        name: None,
    }));
    assert!(
        second.contains(&space.to_string()) && second.contains("Cabinet 206"),
        "{second}"
    );
    let not_location = rejected(e.execute(&CreateLayoutSpace {
        location: FUNC_P01,
        name: None,
    }));
    assert!(not_location.contains("not a location"), "{not_location}");
    assert_eq!(
        pointer(e.execute(&CreateLayoutSpace {
            location: id(0xeeee),
            name: None
        })),
        "/location"
    );
    let delete_node = DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::StructureNode,
            id: LOC_206.cast(),
        }],
    };
    assert_eq!(referenced(e.execute(&delete_node)), "layout_space.location");
    e.execute(&DeleteLayoutSpace { space }).unwrap();
    assert!(rows::<LayoutSpace>(&e).is_empty());
    e.execute(&delete_node).unwrap();
}

/// LM-3: -UC1's enclosure part (750 × 1000 × 420 mm, plate 650 × 900 mm) gives one enclosure and one plate surface
/// whose device is the child carrying the plate, its frame at (50, 50, 0) mm and its usable rectangle (0, 0)–(650, 900)
/// mm; a device without an enclosure part is refused with D6's text; a second enclosure in the space is refused.
#[test]
fn lm3_an_enclosure_brings_its_plate() {
    let mut e = engine(model());
    let space = created(
        e.execute(&CreateLayoutSpace {
            location: LOC,
            name: None,
        })
        .unwrap(),
    );
    let none = rejected(e.execute(&CreateEnclosure {
        space,
        device: DEV_K1,
        pose: None,
        pe_bond: None,
    }));
    assert!(none.ends_with("the device has no enclosure part"), "{none}");
    let enclosure: Id<Enclosure> = created(
        e.execute(&CreateEnclosure {
            space,
            device: DEV_UC1,
            pose: None,
            pe_bond: None,
        })
        .unwrap(),
    );
    let encl = get(&e, enclosure).unwrap();
    assert_eq!(
        (encl.space, encl.device, encl.pose),
        (space, DEV_UC1, Pose3::IDENTITY)
    );
    let surfaces = rows::<MountingSurface>(&e);
    assert_eq!(surfaces.len(), 1);
    let s = &surfaces[0];
    assert_eq!(
        (s.enclosure, s.kind.clone(), s.device, s.pe_bond),
        (enclosure, SurfaceKind::Plate, Some(DEV_PLATE), false)
    );
    assert_eq!(
        s.frame,
        Pose3 {
            t: [Nm(50 * MM), Nm(50 * MM), Nm(0)],
            q: Quat9::IDENTITY
        }
    );
    assert_eq!(s.usable, vec![p(0, 0), p(650, 0), p(650, 900), p(0, 900)]);
    let second = rejected(e.execute(&CreateEnclosure {
        space,
        device: DEV_UC1,
        pose: None,
        pe_bond: None,
    }));
    assert!(
        second.contains("already has enclosure") && second.contains("UC1"),
        "{second}"
    );
    assert_eq!(e.history().cursor(), 2, "one step each");
}

/// LM-4: a rail (two points, its profile from its part's rail section) and a 40 × 60 mm duct on an orthogonal polyline
/// are added; a profile that differs from the part's, a diagonal segment and the wrong parameters are refused at their
/// pointer.
#[test]
fn lm4_carriers_are_axis_parallel() {
    let Layout {
        mut e, surface, rail, ..
    } = layout();
    let r = get(&e, rail).unwrap();
    assert_eq!(r.path, vec![p(20, 800), p(620, 800)]);
    assert_eq!(r.part, Some(PartRef::new(copy_id(ITEM_RAIL35))));
    assert!(r.pe_bond);
    let duct = AddCarrier {
        surface,
        kind: CarrierKindParam::Duct,
        part: None,
        path: vec![p(0, 880), p(650, 880), p(650, 20)],
        params: CarrierParamsParam::Duct {
            width: Nm(40 * MM),
            height: Nm(60 * MM),
            slot_pitch: None,
            cover: Some(true),
        },
        pe_bond: None,
    };
    let d: Id<Carrier> = created(e.execute(&duct).unwrap());
    assert_eq!(get(&e, d).unwrap().kind, ew_schema::CarrierKind::Duct);
    let wrong_profile = rail_cmd(surface, 700, ITEM_RAIL35, MountKind::Ts32);
    assert_eq!(pointer(e.execute(&wrong_profile)), "/params/rail/profile");
    let mut diagonal = rail_cmd(surface, 700, ITEM_RAIL35, MountKind::Ts35x7_5);
    diagonal.path = vec![p(20, 700), p(620, 720)];
    assert_eq!(pointer(e.execute(&diagonal)), "/path");
    let mut bent = duct.clone();
    bent.path = vec![p(0, 0), p(100, 0), p(150, 100)];
    assert_eq!(pointer(e.execute(&bent)), "/path");
    let mut mixed = duct;
    mixed.kind = CarrierKindParam::Rail;
    assert_eq!(pointer(e.execute(&mixed)), "/params");
}

/// LM-4: an occupied rail is refused without `then`; with `remove_placements` the preview lists the placement and the
/// rail and the commit is one step (undone in one); with `rehome_to_surface` the placement moves to the plate at the
/// equivalent surface pose (the 18 × 90 mm MCB at s = 96 mm on the rail from (20, 800) mm: origin (107, 755) mm).
#[test]
fn lm4_an_occupied_rail_goes_only_with_then() {
    let Layout {
        mut e, surface, rail, ..
    } = layout();
    let pl: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(96)))
            .unwrap(),
    );
    let refused = rejected(e.execute(&DeleteCarrier {
        carrier: rail,
        then: None,
    }));
    assert!(refused.contains("give `then`"), "{refused}");
    let cursor = e.history().cursor();
    let remove = DeleteCarrier {
        carrier: rail,
        then: Some(CarrierThen::RemovePlacements),
    };
    let preview = e.preview(&remove).unwrap();
    let removed: Vec<_> = preview
        .ops()
        .iter()
        .filter(|o| o.after().is_none())
        .map(|o| o.key())
        .collect();
    assert!(
        removed.contains(&(Kind::Placement, pl.cast())) && removed.contains(&(Kind::Carrier, rail.cast()))
    );
    e.commit(preview).unwrap();
    assert_eq!(e.history().cursor(), cursor + 1);
    assert!(get(&e, pl).is_none() && get(&e, rail).is_none());
    e.undo(&Origin::Gui).unwrap();
    assert!(get(&e, pl).is_some() && get(&e, rail).is_some());

    e.execute(&DeleteCarrier {
        carrier: rail,
        then: Some(CarrierThen::RehomeToSurface),
    })
    .unwrap();
    let moved = get(&e, pl).unwrap();
    assert_eq!(moved.carrier, CarrierRef::Surface(surface));
    assert_eq!(
        moved.pose,
        Pose3 {
            t: [Nm(107 * MM), Nm(755 * MM), Nm(0)],
            q: Quat9::IDENTITY
        }
    );
    assert_eq!(
        moved.part,
        Some(PartRef::new(copy_id(ITEM_MCB))),
        "the part stays"
    );
    assert!(get(&e, rail).is_none());
}

/// LM-5: place, move, lock and remove are one undo step each; `part` is the device's part, or for a device with an
/// accessory and two mountable part uses the first mountable one.
#[test]
fn lm5_placements_are_one_step_each() {
    let Layout {
        mut e, surface, rail, ..
    } = layout();
    let start = e.history().cursor();
    let k1: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(0)))
            .unwrap(),
    );
    let row = get(&e, k1).unwrap();
    assert_eq!(
        (row.device, row.part.clone(), row.carrier.clone(), row.locked),
        (
            DEV_K1,
            Some(PartRef::new(copy_id(ITEM_MCB))),
            CarrierRef::Carrier(rail),
            false
        )
    );
    let on_plate = Pose3 {
        t: [Nm(300 * MM), Nm(100 * MM), Nm(0)],
        q: Quat9::new(707_106_781, 0, 0, 707_106_781).unwrap(),
    };
    let k2: Id<Placement> = created(
        e.execute(&place(DEV_K2, PlaceOnParam::Surface(surface), on_plate))
            .unwrap(),
    );
    assert_eq!(
        get(&e, k2).unwrap().part,
        Some(PartRef::new(copy_id(ITEM_MCB_WIDE)))
    );
    e.execute(&MovePlacement {
        placement: k1,
        carrier: None,
        pose: at(200),
    })
    .unwrap();
    assert_eq!(get(&e, k1).unwrap().pose, at(200));
    e.execute(&LockPlacement {
        placement: k1,
        locked: true,
    })
    .unwrap();
    assert!(get(&e, k1).unwrap().locked);
    e.execute(&RemovePlacement { placement: k2 }).unwrap();
    assert!(get(&e, k2).is_none());
    assert_eq!(e.history().cursor(), start + 5, "five commands, five steps");
    for _ in 0..5 {
        e.undo(&Origin::Gui).unwrap();
    }
    assert!(rows::<Placement>(&e).is_empty());
}

/// LM-5: a TS 35 device on a TS 32 rail, a pose off the rail or off its axis, a second placement of one (device, part,
/// function) and a part the device does not use are each refused; so is a move onto an unfit rail.
#[test]
fn lm5_placement_refusals() {
    let Layout {
        mut e,
        surface,
        rail,
        rail32,
        ..
    } = layout();
    let unfit = rejected(e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail32), at(10))));
    assert!(
        unfit.ends_with("K1 (TS 35×7.5) does not fit a TS 32 rail"),
        "{unfit}"
    );
    e.execute(&place(DEV_K3, PlaceOnParam::Carrier(rail32), at(10)))
        .unwrap();
    let off = rejected(e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(601))));
    assert_eq!(off, "the pose is off the rail: s = 601 mm is outside [0, 600 mm]");
    let mut lifted = at(10);
    lifted.t[1] = Nm(5 * MM);
    let axis = rejected(e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), lifted)));
    assert!(axis.contains("t = (s, 0, 0)"), "{axis}");
    let mut quarter = at(10);
    quarter.q = Quat9::new(707_106_781, 0, 0, 707_106_781).unwrap();
    rejected(e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), quarter)));
    let mut flipped = at(10);
    flipped.q = Quat9::new(0, 0, 0, 1_000_000_000).unwrap();
    let k1: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), flipped))
            .unwrap(),
    );
    let twice = rejected(e.execute(&place(DEV_K1, PlaceOnParam::Surface(surface), Pose3::IDENTITY)));
    assert!(twice.contains("already placed"), "{twice}");
    let mut not_used = place(DEV_K1, PlaceOnParam::Carrier(rail), at(100));
    not_used.part = Some(copy_id(ITEM_G_RAIL));
    assert_eq!(pointer(e.execute(&not_used)), "/part");
    let mut foreign = place(DEV_K1, PlaceOnParam::Carrier(rail), at(100));
    foreign.function = Some(FN_101F1);
    assert_eq!(pointer(e.execute(&foreign)), "/function");
    assert_eq!(
        pointer(e.execute(&place(id(0xeeee), PlaceOnParam::Carrier(rail), at(1)))),
        "/device"
    );
    assert_eq!(
        pointer(e.execute(&place(DEV_K2, PlaceOnParam::Carrier(id(0xeeee)), at(1)))),
        "/carrier"
    );
    let onto = rejected(e.execute(&MovePlacement {
        placement: k1,
        carrier: Some(PlaceOnParam::Carrier(rail32)),
        pose: at(10),
    }));
    assert!(onto.contains("does not fit a TS 32 rail"), "{onto}");
    // the explicit part of another use of K2 is recorded with that use's reference
    let mut explicit = place(DEV_K2, PlaceOnParam::Carrier(rail), at(300));
    explicit.part = Some(copy_id(ITEM_MCB));
    let k2: Id<Placement> = created(e.execute(&explicit).unwrap());
    assert_eq!(get(&e, k2).unwrap().part, Some(PartRef::new(copy_id(ITEM_MCB))));
}

/// LM-6: deleting a placed device removes its placement in the same transaction, listed in the preview; an occupied
/// carrier cannot go with its enclosure or space, and an enclosure's device cannot be deleted; the plate's device is
/// unlinked from its surface.
#[test]
fn lm6_a_placement_never_outlives_its_device() {
    let Layout {
        mut e,
        space,
        enclosure,
        surface,
        rail,
        ..
    } = layout();
    let pl: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(50)))
            .unwrap(),
    );
    assert_eq!(
        referenced(e.execute(&DeleteEnclosure { enclosure })),
        "placement.carrier.carrier"
    );
    assert_eq!(
        referenced(e.execute(&DeleteLayoutSpace { space })),
        "placement.carrier.carrier"
    );
    let delete = |d: Id<Device>| DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Device,
            id: d.cast(),
        }],
    };
    // the plate's device is unlinked from its surface; then -UC1 has no child, and its enclosure keeps it
    e.execute(&delete(DEV_PLATE)).unwrap();
    assert_eq!(get(&e, surface).unwrap().device, None, "unlinked");
    assert_eq!(referenced(e.execute(&delete(DEV_UC1))), "enclosure.device");
    let preview = e.preview(&delete(DEV_K1)).unwrap();
    assert!(
        preview
            .ops()
            .iter()
            .any(|o| o.key() == (Kind::Placement, pl.cast()) && o.after().is_none()),
        "the preview lists the placement"
    );
    e.commit(preview).unwrap();
    assert!(get(&e, pl).is_none());
    e.execute(&DeleteLayoutSpace { space }).unwrap();
    assert!(
        rows::<Enclosure>(&e).is_empty()
            && rows::<MountingSurface>(&e).is_empty()
            && rows::<Carrier>(&e).is_empty()
    );
}

/// LM-7: `part.assign replace` of the placed copy by a new one moves the placement's `part` in the same step;
/// `replace_all` by a copy the device already uses removes the placed copy without adding one, and the placement keeps
/// its `part` (EW-LAY-003 reports it).
#[test]
fn lm7_a_part_change_follows_the_placement() {
    let Layout { mut e, rail, .. } = layout();
    let k1: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(50)))
            .unwrap(),
    );
    let cursor = e.history().cursor();
    e.execute(&AssignPart {
        devices: vec![DEV_K1],
        part: copy_id(ITEM_MCB_WIDE),
        copies: vec![],
        mode: AssignModeParam::Replace,
        from: Some(copy_id(ITEM_MCB)),
        qty: None,
    })
    .unwrap();
    assert_eq!(e.history().cursor(), cursor + 1);
    assert_eq!(
        get(&e, k1).unwrap().part,
        Some(PartRef::new(copy_id(ITEM_MCB_WIDE)))
    );
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(get(&e, k1).unwrap().part, Some(PartRef::new(copy_id(ITEM_MCB))));

    let k2: Id<Placement> = created(
        e.execute(&place(DEV_K2, PlaceOnParam::Carrier(rail), at(300)))
            .unwrap(),
    );
    assert_eq!(
        get(&e, k2).unwrap().part,
        Some(PartRef::new(copy_id(ITEM_MCB_WIDE)))
    );
    e.execute(&AssignPart {
        devices: vec![DEV_K2],
        part: copy_id(ITEM_MCB),
        copies: vec![],
        mode: AssignModeParam::ReplaceAll,
        from: None,
        qty: None,
    })
    .unwrap();
    assert_eq!(get(&e, DEV_K2).unwrap().parts, uses(&[ITEM_MCB]));
    assert_eq!(
        get(&e, k2).unwrap().part,
        Some(PartRef::new(copy_id(ITEM_MCB_WIDE))),
        "kept"
    );
}

/// A fork of a part copy (`part.edit_copy`, D20) re-points a carrier's part (a part-copy link) and a placement's
/// `part` (the use it realises, D4 and D7) with the device's part uses.
#[test]
fn a_part_fork_repoints_carriers_and_placements() {
    let Layout { mut e, rail, .. } = layout();
    let pl: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(50)))
            .unwrap(),
    );
    let project = e.model().project().id.as_uuid();
    for (item, fork) in [(ITEM_MCB, 0xf001_u128), (ITEM_RAIL35, 0xf002)] {
        let original = e
            .model()
            .table::<LibCopy<Part>>()
            .get(&copy_id(item))
            .map(|c| (**c).clone())
            .unwrap();
        let mut part = serde_json::to_value(&original.item).unwrap();
        part["description"] = serde_json::json!("forked");
        let cmd = EditPartCopy {
            copy: copy_id(item),
            part,
            fork: Some(Id::from_uuid(uuid(fork))),
            copies: Vec::new(),
        };
        let preview = e.preview(&cmd).unwrap();
        e.commit(preview).unwrap();
    }
    let fork = |n: u128| -> Id<Part> { v5::lib_copy_id(project, uuid(n)) };
    assert_eq!(
        get(&e, DEV_K1).unwrap().parts,
        vec![PartUse {
            part: PartRef::new(fork(0xf001)),
            qty: Dec3::ONE,
            extra: Default::default(),
        }]
    );
    assert_eq!(get(&e, pl).unwrap().part, Some(PartRef::new(fork(0xf001))));
    assert_eq!(get(&e, rail).unwrap().part, Some(PartRef::new(fork(0xf002))));
}

/// `layout.carrier.edit`: moving a rail to another space's plate carries its placements (their poses are relative to
/// it); a member-less edit and a path at another angle are refused; `part: null` removes the part.
#[test]
fn a_carrier_edit_moves_its_placements_with_it() {
    let Layout { mut e, rail, .. } = layout();
    let space2 = created(
        e.execute(&CreateLayoutSpace {
            location: LOC_206,
            name: None,
        })
        .unwrap(),
    );
    let encl2: Id<Enclosure> = created(
        e.execute(&CreateEnclosure {
            space: space2,
            device: DEV_UC2,
            pose: None,
            pe_bond: None,
        })
        .unwrap(),
    );
    let surface2 = rows::<MountingSurface>(&e)
        .into_iter()
        .find(|s| s.enclosure == encl2)
        .expect("the second plate");
    assert_eq!(surface2.device, None, "-UC20 has no plate child");
    let pl: Id<Placement> = created(
        e.execute(&place(DEV_K1, PlaceOnParam::Carrier(rail), at(50)))
            .unwrap(),
    );
    let empty = EditCarrier {
        carrier: rail,
        surface: None,
        part: None,
        path: None,
        params: None,
        pe_bond: None,
    };
    assert_eq!(pointer(e.execute(&empty)), "");
    let mut bad = empty.clone();
    bad.path = Some(vec![p(0, 0), p(10, 10)]);
    assert_eq!(pointer(e.execute(&bad)), "/path");
    let mut moved = empty.clone();
    moved.surface = Some(surface2.id);
    moved.path = Some(vec![p(20, 700), p(640, 700)]);
    e.execute(&moved).unwrap();
    let r = get(&e, rail).unwrap();
    assert_eq!((r.surface, r.path[1]), (surface2.id, p(640, 700)));
    let after = get(&e, pl).unwrap();
    assert_eq!(
        (after.carrier.clone(), after.pose),
        (CarrierRef::Carrier(rail), at(50)),
        "untouched, moved along"
    );
    let mut unpart = empty;
    unpart.part = Some(None);
    e.execute(&unpart).unwrap();
    assert_eq!(get(&e, rail).unwrap().part, None);
}

/// Every layout command's parameters round-trip through their JSON form (the journal form).
#[test]
fn layout_commands_round_trip_as_json() {
    let surface: Id<MountingSurface> = id(0xb1);
    let rail: Id<Carrier> = id(0xb2);
    let json = serde_json::to_value(place(DEV_K1, PlaceOnParam::Carrier(rail), at(96))).unwrap();
    assert_eq!(
        json["carrier"],
        serde_json::json!({ "carrier": rail.to_string() })
    );
    assert_eq!(
        json["pose"],
        serde_json::json!({"t": [96_000_000, 0, 0], "q": [1_000_000_000, 0, 0, 0]})
    );
    let back: PlaceDevice = serde_json::from_value(json).unwrap();
    assert_eq!(back.carrier, PlaceOnParam::Carrier(rail));
    let edit: EditCarrier =
        serde_json::from_value(serde_json::json!({"carrier": rail.to_string(), "part": null})).unwrap();
    assert_eq!(edit.part, Some(None));
    assert_eq!(
        serde_json::to_value(&edit).unwrap()["part"],
        serde_json::Value::Null
    );
    let add = rail_cmd(surface, 800, ITEM_RAIL35, MountKind::Ts35x7_5);
    let v = serde_json::to_value(&add).unwrap();
    assert_eq!(v["params"], serde_json::json!({"rail": {"profile": "ts35x7_5"}}));
    assert_eq!(serde_json::from_value::<AddCarrier>(v).unwrap(), add);
    assert_eq!(PlaceDevice::NAME, "layout.place");
}
