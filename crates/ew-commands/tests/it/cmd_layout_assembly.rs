// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.02 part 2 at the command layer, on a fixture: `layout.place_assembly` and `layout.auto_fill` (D6),
//! `layout.enclosure.create_from_part` and `layout.move {part}` (D7), the FT-840 edits and `edit.delete` on the layout
//! kinds (FT-837). Every refusal is asserted whole; each command is one undo step. Golden A's numbers (AS-1 to AS-3)
//! are `ew-api`'s `tests/it/golden_layout_assembly.rs`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddCarrier, AttachChildren, AutoFill, CarrierKindParam, CarrierParamsParam, ChildSpec, CreateEnclosure,
    CreateEnclosureFromPart, CreateLayoutSpace, DeleteSelection, EditEnclosure, EditLayoutSpace, EditSurface,
    FillOrder, KindName, MovePlacement, PageScope, PlaceAssembly, PlaceDevice, PlaceMode, PlaceOnParam,
    RemovePlacement, RenumberPages, SetFields, TargetRef,
};
use ew_commands::{CmdError, Command, Cost, Engine, Origin};
use ew_derive::Q;
use ew_derive::layout::member_order;
use ew_ids::{Id, v5};
use ew_model::Model;
use ew_rds::Aspect;
use ew_schema::{
    Carrier, Clearance, Device, Enclosure, EnclosureSection, FnRole, FrozenTag, Kind, LayoutSpace, LibCopy,
    LibDocument, MechData, MountKind, MountingSurface, Part, PartRef, PartUse, Placement, PlateSection,
    RailSection, Record, Slot, StructureNode, SurfaceKind, TagScheme, TagSign, TagSpec,
};
use ew_units::{Dec3, Nm, Pose3, Pt2, Quat9};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

const LIB_A: u128 = 0xd200;
const ITEM_ENCL: u128 = 0xd201;
const ITEM_PLATE: u128 = 0xd202;
const ITEM_RAIL: u128 = 0xd203;
const ITEM_MCB: u128 = 0xd204; // TS 35, 18 × 90 mm
const ITEM_WIDE: u128 = 0xd205; // TS 35, 40 × 90 mm
const ITEM_TS32: u128 = 0xd206; // TS 32, 12 × 60 mm
const ITEM_SCREW: u128 = 0xd207; // screw-mounted, 50 × 50 mm
const ITEM_PAD: u128 = 0xd208; // TS 35, 18 × 90 mm, clearance 5 mm left and right

const DEV_UC1: Id<Device> = id(0xa00);
const DEV_PLATE: Id<Device> = id(0xa01);
/// The assembly A1 with the members M1 (position 2), M2 (position 3) and M3 (position 1): its member order is A1, M3,
/// M1, M2, which the natural order of the tags alone would not give.
const DEV_A1: Id<Device> = id(0xb00);
const DEV_M1: Id<Device> = id(0xb01);
const DEV_M2: Id<Device> = id(0xb02);
const DEV_M3: Id<Device> = id(0xb03);
/// K1 … K8: single 18 mm devices; K4 … K7 have a main representation (schematic order).
const fn k(n: u128) -> Id<Device> {
    id(0xb10 + n)
}
/// K9: a device of another location.
const DEV_K9: Id<Device> = id(0xb19);
const DEV_X5: Id<Device> = id(0xb20);
const DEV_E55: Id<Device> = id(0xb21);
/// W1: a device without functions (exempt).
const DEV_W1: Id<Device> = id(0xb22);
/// The assembly B1 whose member B2 is TS 32.
const DEV_B1: Id<Device> = id(0xb30);
const DEV_B2: Id<Device> = id(0xb31);
/// KM: a device with three parts (18 mm, 40 mm, TS 32), for `layout.move {part}`.
const DEV_KM: Id<Device> = id(0xb40);
/// The many devices of the bulk fill.
const fn many(n: u128) -> Id<Device> {
    id(0x1_0000 + n)
}
const LOC_206: Id<StructureNode> = id(0xa10);

fn copy_id(item: u128) -> Id<Part> {
    v5::lib_copy_id(uuid(LIB_A), uuid(item))
}

fn copy(item: u128, number: &str) -> LibCopy<Part> {
    let mut c = part(copy_id(item), number);
    c.library_id = Id::from_uuid(uuid(LIB_A));
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

fn enclosure_copy() -> LibCopy<Part> {
    let mut encl = copy(ITEM_ENCL, "NSYS3D10840P");
    encl.item.sections.enclosure = Some(EnclosureSection {
        width: Some(Nm(750 * MM)),
        height: Some(Nm(1000 * MM)),
        depth: Some(Nm(420 * MM)),
        plate: Some(lib_ref_to(uuid(LIB_A), uuid(ITEM_PLATE))),
        ..EnclosureSection::default()
    });
    encl
}

fn plate_copy() -> LibCopy<Part> {
    let mut plate = copy(ITEM_PLATE, "NSYMM108");
    plate.item.sections.mounting_plate = Some(PlateSection {
        width: Some(Nm(650 * MM)),
        height: Some(Nm(900 * MM)),
        ..PlateSection::default()
    });
    plate
}

fn device_copies() -> Vec<LibCopy<Part>> {
    let mut rail = copy(ITEM_RAIL, "RAIL");
    rail.item.sections.rail = Some(RailSection {
        profile: MountKind::Ts35x7_5,
        stock_length: None,
        extra: Default::default(),
    });
    let device = |item, number, m| {
        let mut c = copy(item, number);
        c.item.mech = Some(m);
        c
    };
    let mut pad = mech(MountKind::Ts35x7_5, 18, 90);
    pad.clearance = Some(Clearance {
        left: Nm(5 * MM),
        right: Nm(5 * MM),
        ..Clearance::default()
    });
    vec![
        rail,
        device(ITEM_MCB, "MCB", mech(MountKind::Ts35x7_5, 18, 90)),
        device(ITEM_WIDE, "WIDE", mech(MountKind::Ts35x7_5, 40, 90)),
        device(ITEM_TS32, "GRAIL", mech(MountKind::Ts32, 12, 60)),
        device(ITEM_SCREW, "SCREW", mech(MountKind::Screw, 50, 50)),
        device(ITEM_PAD, "PAD", pad),
    ]
}

fn uses(items: &[u128]) -> Vec<PartUse> {
    items
        .iter()
        .map(|i| PartUse {
            part: PartRef::new(copy_id(*i)),
            qty: Dec3::ONE,
            extra: Default::default(),
        })
        .collect()
}

/// What the fixture model holds.
struct Spec {
    /// The enclosure device -UC1 and its plate child.
    uc1: bool,
    /// The enclosure and plate part copies.
    enclosure_copies: bool,
    /// How many extra 18 mm devices (the bulk fill).
    many: u32,
}

const FULL: Spec = Spec {
    uc1: true,
    enclosure_copies: true,
    many: 0,
};

fn model(spec: &Spec) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    // The pages without a form take the default frame grid, so that a representation has an anchor cell (AS-4).
    for r in &mut records {
        if let Record::Page(page) = r {
            page.form = None;
        }
    }
    let mut fid = 0xc00u128;
    let mut with_function = |records: &mut Vec<Record>, d: Device| {
        fid += 1;
        records.push(Record::Function(function(id(fid), d.id, FNDEF_MCB, FnRole::Main)));
        records.push(Record::Device(d));
    };
    if spec.uc1 {
        let mut uc1 = device(DEV_UC1, "UC", 1);
        uc1.parts = uses(&[ITEM_ENCL]);
        let mut plate = device(DEV_PLATE, "UC", 2);
        plate.parent = Some(DEV_UC1);
        plate.parts = uses(&[ITEM_PLATE]);
        records.extend([uc1, plate].map(Record::Device));
    }
    let mut a1 = device(DEV_A1, "A", 1);
    a1.parts = uses(&[ITEM_MCB]);
    with_function(&mut records, a1);
    for (dev, n, position) in [(DEV_M1, 1, 2), (DEV_M2, 2, 3), (DEV_M3, 3, 1)] {
        let mut m = device(dev, "M", n);
        m.parent = Some(DEV_A1);
        m.position = Some(position);
        m.parts = uses(&[ITEM_MCB]);
        with_function(&mut records, m);
    }
    for n in 1..=8u32 {
        let mut d = device(k(u128::from(n)), "K", n);
        d.parts = uses(&[ITEM_MCB]);
        with_function(&mut records, d);
    }
    let mut k9 = device(DEV_K9, "K", 9);
    k9.structure.location = Some(Slot::Node(LOC_206));
    k9.parts = uses(&[ITEM_MCB]);
    with_function(&mut records, k9);
    let mut x5 = device(DEV_X5, "X", 5);
    x5.parts = uses(&[ITEM_TS32]);
    with_function(&mut records, x5);
    let mut e55 = device(DEV_E55, "E", 55);
    e55.parts = uses(&[ITEM_SCREW]);
    with_function(&mut records, e55);
    let mut w1 = device(DEV_W1, "W", 1);
    w1.parts = uses(&[ITEM_MCB]);
    records.push(Record::Device(w1));
    let mut b1 = device(DEV_B1, "B", 1);
    b1.parts = uses(&[ITEM_MCB]);
    with_function(&mut records, b1);
    let mut b2 = device(DEV_B2, "B", 2);
    b2.parent = Some(DEV_B1);
    b2.position = Some(1);
    b2.parts = uses(&[ITEM_TS32]);
    with_function(&mut records, b2);
    let mut km = device(DEV_KM, "KM", 1);
    km.parts = uses(&[ITEM_MCB, ITEM_WIDE, ITEM_TS32]);
    with_function(&mut records, km);
    for n in 1..=spec.many {
        let mut d = device(many(u128::from(n)), "Q", n);
        d.parts = uses(&[ITEM_MCB]);
        with_function(&mut records, d);
    }
    // K4 … K7's main representations: K5 on page 101, K4 and K6 on page 102 at two cells, K7 none (K8 neither).
    for (rep_n, dev_n, page, (x, y)) in [
        (1u128, 4u128, PAGE_102, (150, 120)),
        (2, 5, PAGE_101, (40, 40)),
        (3, 6, PAGE_102, (60, 60)),
    ] {
        let f = records
            .iter()
            .find_map(|r| match r {
                Record::Function(f) if f.device == Some(k(dev_n)) => Some(f.id),
                _ => None,
            })
            .expect("the device's function");
        let mut r = rep(id(0xe00 + rep_n), page, vec![f]);
        r.pos = Pt2::new(Nm(x * MM), Nm(y * MM));
        records.push(Record::Representation(r));
    }
    records.push(Record::StructureNode(structure_node(
        LOC_206,
        Aspect::Location,
        "J-E-206",
        None,
    )));
    let mut lib = library();
    lib.extend(device_copies().into_iter().map(LibDocument::Part));
    if spec.enclosure_copies {
        lib.extend([enclosure_copy(), plate_copy()].map(LibDocument::Part));
    }
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

fn rail_cmd(surface: Id<MountingSurface>, y: i64, length: i64) -> AddCarrier {
    AddCarrier {
        surface,
        kind: CarrierKindParam::Rail,
        part: Some(copy_id(ITEM_RAIL)),
        path: vec![p(20, y), p(20 + length, y)],
        params: CarrierParamsParam::Rail {
            profile: MountKind::Ts35x7_5,
        },
        pe_bond: None,
        copies: vec![],
    }
}

fn created<T>(r: ew_commands::Receipt) -> Id<T> {
    r.outcome.primary.expect("a primary record").1.cast()
}

fn rejected(r: Result<ew_commands::Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn invalid(r: Result<ew_commands::Receipt, CmdError>) -> (String, String) {
    match r {
        Err(CmdError::InvalidParams { path, message, .. }) => (path, message),
        other => panic!("expected invalid parameters, got {other:?}"),
    }
}

fn get<T: ew_model::Entity<Marker = T> + Clone>(e: &Engine, id: Id<T>) -> Option<T> {
    e.model().table::<T>().get(&id).map(|r| (**r).clone())
}

fn rows<T: ew_model::Entity + Clone>(e: &Engine) -> Vec<T> {
    e.model()
        .table::<T>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .collect()
}

/// The layout of the fixture: the space of +J-E-2416, -UC1's enclosure and plate, rail 1 (y = 800 mm) and rail 2
/// (y = 600 mm), both TS 35 and 600 mm long, a 30 mm TS 35 rail 3 (y = 400 mm) and a duct.
struct Fx {
    e: Engine,
    space: Id<LayoutSpace>,
    enclosure: Id<Enclosure>,
    surface: Id<MountingSurface>,
    rail1: Id<Carrier>,
    rail2: Id<Carrier>,
    rail3: Id<Carrier>,
    duct: Id<Carrier>,
}

fn fx_with(spec: &Spec) -> Fx {
    let mut e = engine(model(spec));
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
            pe_bond: None,
        })
        .unwrap(),
    );
    let surface = rows::<MountingSurface>(&e)[0].id;
    let rail1 = created(e.execute(&rail_cmd(surface, 800, 600)).unwrap());
    let rail2 = created(e.execute(&rail_cmd(surface, 600, 600)).unwrap());
    let rail3 = created(e.execute(&rail_cmd(surface, 400, 30)).unwrap());
    let duct = created(
        e.execute(&AddCarrier {
            surface,
            kind: CarrierKindParam::Duct,
            part: None,
            path: vec![p(0, 880), p(650, 880)],
            params: CarrierParamsParam::Duct {
                width: Nm(40 * MM),
                height: Nm(60 * MM),
                slot_pitch: None,
                cover: None,
            },
            pe_bond: None,
            copies: vec![],
        })
        .unwrap(),
    );
    Fx {
        e,
        space,
        enclosure,
        surface,
        rail1,
        rail2,
        rail3,
        duct,
    }
}

fn fx() -> Fx {
    fx_with(&FULL)
}

fn place(device: Id<Device>, rail: Id<Carrier>, s: i64) -> PlaceDevice {
    PlaceDevice {
        device,
        part: None,
        function: None,
        carrier: PlaceOnParam::Carrier(rail),
        pose: at(s),
        mode: None,
    }
}

fn assembly(device: Id<Device>, rail: Id<Carrier>, s: Option<i64>, mode: Option<PlaceMode>) -> PlaceAssembly {
    PlaceAssembly {
        device,
        carrier: PlaceOnParam::Carrier(rail),
        s: s.map(|s| Nm(s * MM)),
        mode,
    }
}

fn fill(rail: Id<Carrier>, devices: Option<Vec<Id<Device>>>, order: FillOrder) -> AutoFill {
    AutoFill {
        carrier: rail,
        devices,
        order,
    }
}

/// The clip point of a device's one placement in tenths of a millimetre.
fn clip10(e: &Engine, device: Id<Device>) -> i64 {
    let placements: Vec<Placement> = rows::<Placement>(e)
        .into_iter()
        .filter(|p| p.device == device)
        .collect();
    assert_eq!(placements.len(), 1, "one placement of {device}");
    placements[0].pose.t[0].0 * 10 / MM
}

fn clips(e: &Engine, devices: &[Id<Device>]) -> Vec<i64> {
    devices.iter().map(|d| clip10(e, *d) / 10).collect()
}

fn placed(e: &Engine, device: Id<Device>) -> bool {
    rows::<Placement>(e).iter().any(|p| p.device == device)
}

/// AS-2 (fixture): with no member on the rail the members go as one block at `s` in member order (position first),
/// abutting; each records its primary part; one undo step.
#[test]
fn as2_a_block_lands_in_member_order() {
    let mut f = fx();
    let before = lines(f.e.model());
    let cursor = f.e.history().cursor();
    let r = f.e.execute(&assembly(DEV_M2, f.rail1, Some(20), None)).unwrap();
    let order = member_order(&Q::new(f.e.model()), DEV_A1);
    assert_eq!(order, vec![DEV_A1, DEV_M3, DEV_M1, DEV_M2]);
    assert_eq!(clips(&f.e, &order), vec![29, 47, 65, 83]);
    let first: Id<Placement> = created(r);
    assert_eq!(
        get(&f.e, first).unwrap().device,
        DEV_A1,
        "the primary is the first new placement"
    );
    for pl in rows::<Placement>(&f.e) {
        assert_eq!(pl.part, Some(PartRef::new(copy_id(ITEM_MCB))));
        assert_eq!(pl.carrier, ew_schema::CarrierRef::Carrier(f.rail1));
        assert!(!pl.locked);
    }
    assert_eq!(f.e.history().cursor(), cursor + 1, "one step");
    f.e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(f.e.model()), before);
    // Placed: nothing left to place.
    f.e.execute(&assembly(DEV_A1, f.rail1, Some(20), None)).unwrap();
    assert_eq!(
        rejected(f.e.execute(&assembly(DEV_M1, f.rail1, Some(20), None))),
        "Every member of -?A1 that mounts is already placed."
    );
}

/// AS-2 (fixture): the block in free mode goes to the nearest gap where it fits; in push mode it stays at `s` and
/// pushes the placement to its right as one item.
#[test]
fn as2_the_block_resolves_as_one_item() {
    let mut f = fx();
    f.e.execute(&place(k(1), f.rail1, 60)).unwrap(); // [51, 69]
    let order = [DEV_A1, DEV_M3, DEV_M1, DEV_M2];
    f.e.execute(&assembly(DEV_A1, f.rail1, Some(20), Some(PlaceMode::Free)))
        .unwrap();
    assert_eq!(
        clips(&f.e, &order),
        vec![78, 96, 114, 132],
        "after K1: the gap before it is 51 mm"
    );
    assert_eq!(clip10(&f.e, k(1)), 600);
    f.e.undo(&Origin::Gui).unwrap();
    f.e.execute(&assembly(DEV_A1, f.rail1, Some(20), Some(PlaceMode::Push)))
        .unwrap();
    assert_eq!(clips(&f.e, &order), vec![29, 47, 65, 83]);
    assert_eq!(clip10(&f.e, k(1)), 1010, "pushed to the block's end, 92 mm");
}

/// AS-2 (fixture), ruling 5 (d): a member without a placed predecessor goes before its first placed successor; each
/// later member after its predecessor, pushing the rest; `s` is not needed.
#[test]
fn as2_members_join_the_placed_ones() {
    let mut f = fx();
    f.e.execute(&place(DEV_M1, f.rail1, 300)).unwrap(); // [291, 309]
    let cursor = f.e.history().cursor();
    f.e.execute(&assembly(DEV_A1, f.rail1, None, None)).unwrap();
    assert_eq!(
        clips(&f.e, &[DEV_A1, DEV_M3, DEV_M1, DEV_M2]),
        vec![282, 300, 318, 336]
    );
    assert_eq!(f.e.history().cursor(), cursor + 1, "one step");
}

/// Override 3 of lane E42: an insertion that pushes a member this command placed earlier re-poses it in memory; it is
/// inserted once, at the pushed pose.
#[test]
fn override3_a_member_pushed_in_the_same_command_lands_once() {
    let mut f = fx();
    f.e.execute(&place(DEV_A1, f.rail1, 47)).unwrap(); // [38, 56]
    f.e.execute(&place(DEV_M1, f.rail1, 29)).unwrap(); // [20, 38]
    f.e.execute(&assembly(DEV_A1, f.rail1, None, None)).unwrap();
    // M3 goes after A1 (65), then M2 after M1 pushes A1 and M3 by 18 mm.
    assert_eq!(
        clips(&f.e, &[DEV_M1, DEV_M2, DEV_A1, DEV_M3]),
        vec![29, 47, 65, 83]
    );
}

/// AS-2 (fixture): every refusal of `layout.place_assembly`; an unfit member refuses the whole command.
#[test]
fn as2_place_assembly_refusals() {
    let mut f = fx();
    let before = lines(f.e.model());
    assert_eq!(
        rejected(f.e.execute(&assembly(DEV_A1, f.rail1, Some(20), Some(PlaceMode::Swap)))),
        "Swap works with one placement; place an assembly in free or push mode."
    );
    assert_eq!(
        rejected(f.e.execute(&assembly(DEV_A1, f.duct, Some(20), None))),
        "An assembly is placed on a rail."
    );
    let on_surface = PlaceAssembly {
        carrier: PlaceOnParam::Surface(f.surface),
        ..assembly(DEV_A1, f.rail1, Some(20), None)
    };
    assert_eq!(
        rejected(f.e.execute(&on_surface)),
        "An assembly is placed on a rail."
    );
    assert_eq!(
        invalid(f.e.execute(&assembly(DEV_A1, f.rail1, None, None))),
        (
            "/s".to_owned(),
            "no member of -?A1 is on rail 1 yet; give the position s".to_owned()
        )
    );
    assert_eq!(
        rejected(f.e.execute(&assembly(k(1), f.rail1, Some(20), None))),
        "-?K1 is not part of an assembly; place it on its own."
    );
    assert_eq!(
        rejected(f.e.execute(&assembly(DEV_B2, f.rail1, Some(20), None))),
        "-?B2 (TS 32) does not fit a TS 35×7.5 rail"
    );
    assert_eq!(
        invalid(f.e.execute(&assembly(id(0xeeee), f.rail1, Some(20), None))).0,
        "/device"
    );
    assert_eq!(
        invalid(f.e.execute(&assembly(DEV_A1, id(0xeeee), Some(20), None))).0,
        "/carrier"
    );
    assert_eq!(lines(f.e.model()), before, "nothing placed");
}

/// Override 2 of lane E42: a concurrent placement of a device the command places conflicts at merge, for
/// `layout.place_assembly` and for `layout.auto_fill`, even on another rail.
#[test]
fn override2_a_concurrent_placement_of_a_member_conflicts() {
    let mut f = fx();
    let preview = f.e.preview(&assembly(DEV_A1, f.rail1, Some(20), None)).unwrap();
    let base = preview.base_rev();
    f.e.execute(&place(DEV_M2, f.rail2, 100)).unwrap();
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the assembly read M2's placements"
    );
    let preview =
        f.e.preview(&fill(f.rail1, Some(vec![k(1), k(2)]), FillOrder::Dt))
            .unwrap();
    let base = preview.base_rev();
    f.e.execute(&place(k(2), f.rail2, 300)).unwrap();
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the fill read K2's placements"
    );
}

/// AS-3 (fixture) and ruling 8: next-fit from the rail start in DT order; one notice per device not placed, in list
/// order, with the table's texts; the outcome names the carrier.
#[test]
fn as3_auto_fill_next_fits_and_reports() {
    let mut f = fx();
    f.e.execute(&place(k(2), f.rail2, 100)).unwrap();
    let cursor = f.e.history().cursor();
    let r =
        f.e.execute(&fill(
            f.rail1,
            Some(vec![k(3), DEV_X5, DEV_E55, k(1), DEV_K9, DEV_W1, k(2)]),
            FillOrder::Dt,
        ))
        .unwrap();
    assert_eq!(r.outcome.primary, Some((Kind::Carrier, f.rail1.cast())));
    assert_eq!(clips(&f.e, &[k(1), k(3)]), vec![9, 27]);
    assert_eq!(
        r.outcome.notices,
        vec![
            "-?X5: TS 32 does not fit a TS 35×7.5 rail".to_owned(),
            "-?E55: screw-mounted, not on a rail".to_owned(),
            "-?K9: not in this rail's layout space".to_owned(),
            "-?W1: not mountable".to_owned(),
            "-?K2: already placed".to_owned(),
        ]
    );
    assert_eq!(f.e.history().cursor(), cursor + 1, "one step");
    // Placed items never move: the next fill starts after them.
    f.e.execute(&fill(f.rail1, Some(vec![k(4)]), FillOrder::Dt))
        .unwrap();
    assert_eq!(clip10(&f.e, k(4)), 450);
}

/// AS-3 (fixture): a part's clearance pads space the fill (D3): 18 mm + 5 mm pads.
#[test]
fn as3_the_pads_space_the_fill() {
    let mut f = fx();
    f.e.execute(&place(k(8), f.rail1, 9)).unwrap(); // [0, 18]
    let padded = ew_commands::commands::AssignPart {
        devices: vec![k(1)],
        part: copy_id(ITEM_PAD),
        mode: ew_commands::commands::AssignModeParam::Replace,
        copies: vec![],
        from: Some(copy_id(ITEM_MCB)),
        qty: None,
    };
    f.e.execute(&padded).unwrap();
    f.e.execute(&fill(f.rail1, Some(vec![k(1), k(2)]), FillOrder::Dt))
        .unwrap();
    // K1 after K8 plus its 5 mm pad: [23, 41]; K2 after K1's 5 mm pad: [46, 64].
    assert_eq!(clips(&f.e, &[k(1), k(2)]), vec![32, 55]);
}

/// AS-3 (fixture): an item that fits nowhere is left with a notice and the fill continues; nothing placed at all, a
/// duct, an empty list and a bad list are refused.
#[test]
fn as3_auto_fill_refusals() {
    let mut f = fx();
    let r =
        f.e.execute(&fill(f.rail3, Some(vec![k(1), k(2)]), FillOrder::Dt))
            .unwrap();
    assert_eq!(clip10(&f.e, k(1)), 90);
    assert_eq!(r.outcome.notices, vec!["-?K2: no room left on rail 3".to_owned()]);
    assert_eq!(
        rejected(f.e.execute(&fill(f.rail1, Some(vec![DEV_X5, k(1)]), FillOrder::Dt))),
        "Nothing was placed on rail 1: -?X5: TS 32 does not fit a TS 35×7.5 rail; -?K1: already placed"
    );
    assert_eq!(
        rejected(f.e.execute(&fill(f.rail1, Some(vec![]), FillOrder::Dt))),
        "Nothing was placed on rail 1: no device to place."
    );
    assert_eq!(
        rejected(f.e.execute(&fill(f.duct, None, FillOrder::Dt))),
        "Auto-fill works on a rail."
    );
    assert_eq!(
        invalid(f.e.execute(&fill(f.rail1, Some(vec![k(2), k(2)]), FillOrder::Dt))),
        ("/devices/1".to_owned(), "the device is listed twice".to_owned())
    );
    assert_eq!(
        invalid(f.e.execute(&fill(f.rail1, Some(vec![id(0xeeee)]), FillOrder::Dt))),
        (
            "/devices/0".to_owned(),
            "the device is not in the project".to_owned()
        )
    );
}

/// The default list: the unplaced devices of the rail's space that fit it; the unfit (TS 32, screw-mounted) are not
/// listed, so they give no notice.
#[test]
fn auto_fill_lists_the_unplaced_that_fit() {
    let mut f = fx();
    let r = f.e.execute(&fill(f.rail1, None, FillOrder::Dt)).unwrap();
    for d in [DEV_X5, DEV_E55, DEV_B2, DEV_K9, DEV_W1] {
        assert!(!placed(&f.e, d), "{d} is not placed");
    }
    for d in [DEV_A1, DEV_M1, k(1), k(8)] {
        assert!(placed(&f.e, d), "{d} is placed");
    }
    assert!(
        r.outcome
            .notices
            .iter()
            .all(|n| n.ends_with(": no room left on rail 1")),
        "only rail capacity: {:?}",
        r.outcome.notices
    );
}

/// AS-4: schematic order is page order, then row, then column of the main representation's anchor cell; devices
/// without one last, by DT.
#[test]
fn as4_schematic_order() {
    let mut f = fx();
    let cell =
        |rep: u128| ew_commands::numbering::anchor_cell_in(f.e.model(), id(0xe00 + rep)).expect("a cell");
    let (k4, k6) = (cell(1), cell(3));
    assert_ne!((k4.row, k4.column), (k6.row, k6.column), "two cells");
    let (first, second) = if (k4.row, k4.column) < (k6.row, k6.column) {
        (k(4), k(6))
    } else {
        (k(6), k(4))
    };
    f.e.execute(&fill(
        f.rail1,
        Some(vec![k(8), k(7), k(6), k(5), k(4)]),
        FillOrder::Schematic,
    ))
    .unwrap();
    // K5 (page 101), the two of page 102 by cell, then K7, K8 by DT.
    assert_eq!(
        clips(&f.e, &[k(5), first, second, k(7), k(8)]),
        vec![9, 27, 45, 63, 81]
    );
}

/// AS-4: an assembly's members travel as one block at the place of the first of them, in member order.
#[test]
fn as4_an_assembly_stays_one_block() {
    let mut f = fx();
    f.e.execute(&fill(
        f.rail1,
        Some(vec![k(1), DEV_M2, DEV_A1, k(2), DEV_M3]),
        FillOrder::Dt,
    ))
    .unwrap();
    // DT: -?A1 first, so its unit (A1, M3, M2 in member order) goes first; -?M2 and -?M3 do not wait for their tags.
    assert_eq!(
        clips(&f.e, &[DEV_A1, DEV_M3, DEV_M2, k(1), k(2)]),
        vec![9, 27, 45, 63, 81]
    );
}

/// AS-4: more than 200 devices in the list is `Bulk`; the engine refuses it without a preview and commits it through
/// one.
#[test]
fn as4_a_large_fill_is_bulk() {
    let mut f = fx_with(&Spec { many: 201, ..FULL });
    let list: Vec<Id<Device>> = (1..=201).map(many).collect();
    let cmd = fill(f.rail1, Some(list.clone()), FillOrder::Dt);
    assert_eq!(cmd.cost(f.e.model()), Cost::Bulk);
    assert_eq!(
        fill(f.rail1, Some(list[..200].to_vec()), FillOrder::Dt).cost(f.e.model()),
        Cost::Interactive
    );
    assert!(matches!(f.e.execute(&cmd), Err(CmdError::NeedsPreview { .. })));
    let preview = f.e.preview(&cmd).unwrap();
    let r = f.e.commit(preview).unwrap();
    let count = rows::<Placement>(&f.e).len();
    assert!(count > 0);
    assert_eq!(
        count + r.outcome.notices.len(),
        201,
        "every device placed or named"
    );
}

/// The JSON form of a part copy in `lib/` form with its true hash (FT-656's pattern).
fn copy_json(c: LibCopy<Part>) -> Value {
    let doc = LibDocument::Part(c);
    let hash = ew_commands::commands::library_copy_hash(&doc).expect("a known kind");
    let mut v = serde_json::to_value(&doc).expect("copy json");
    v["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    v
}

fn from_part(copies: Vec<Value>) -> CreateEnclosureFromPart {
    CreateEnclosureFromPart {
        location: LOC,
        part: copy_id(ITEM_ENCL),
        copies,
        function: Some(FUNC_P01),
        pose: None,
        pe_bond: None,
    }
}

/// AS-5: from the enclosure part and its plate passed as copies: one space, the enclosure device frozen UC1 with its
/// part, the plate child frozen UC1-plate, one enclosure and one plate surface (frame (50, 50, 0) mm, usable
/// (0, 0)–(650, 900) mm, its device the child); one undo removes all and the embedded copies; a second call refused.
#[test]
fn as5_an_enclosure_from_its_part() {
    let mut e = engine(model(&Spec {
        uc1: false,
        enclosure_copies: false,
        many: 0,
    }));
    let before = lines(e.model());
    let cursor = e.history().cursor();
    let cmd = from_part(vec![copy_json(enclosure_copy()), copy_json(plate_copy())]);
    let enclosure: Id<Enclosure> = created(e.execute(&cmd).unwrap());
    assert_eq!(e.history().cursor(), cursor + 1, "one step");
    let spaces = rows::<LayoutSpace>(&e);
    assert_eq!(spaces.len(), 1);
    assert_eq!((spaces[0].location, spaces[0].name.clone()), (LOC, None));
    let encl = get(&e, enclosure).unwrap();
    assert_eq!((encl.space, encl.pose), (spaces[0].id, Pose3::IDENTITY));
    let device = get(&e, encl.device).unwrap();
    let frozen = |text: &str| {
        Some(FrozenTag {
            text: ident(text),
            sign: TagSign::Shown,
            extra: Default::default(),
        })
    };
    assert_eq!(
        device.tag,
        TagSpec {
            letter: ident("U"),
            counter: Some(1),
            scheme: TagScheme::Default,
            frozen: frozen("UC1"),
            extra: Default::default(),
        }
    );
    assert_eq!(device.parent, None);
    assert_eq!(device.parts, uses(&[ITEM_ENCL]));
    assert_eq!(device.structure.location, Some(Slot::Node(LOC)));
    assert_eq!(device.structure.function, Some(Slot::Node(FUNC_P01)));
    let surfaces = rows::<MountingSurface>(&e);
    assert_eq!(surfaces.len(), 1);
    let s = &surfaces[0];
    let child = get(&e, s.device.expect("the plate's device")).unwrap();
    assert_eq!(
        child.tag,
        TagSpec {
            letter: ident("U"),
            counter: None,
            scheme: TagScheme::Child,
            frozen: frozen("UC1-plate"),
            extra: Default::default(),
        }
    );
    assert_eq!(child.parent, Some(device.id));
    assert_eq!(child.parts, uses(&[ITEM_PLATE]));
    assert_eq!(child.structure, device.structure);
    assert_eq!(
        (s.enclosure, s.kind.clone(), s.pe_bond),
        (enclosure, SurfaceKind::Plate, false)
    );
    assert_eq!(
        s.frame,
        Pose3 {
            t: [Nm(50 * MM), Nm(50 * MM), Nm(0)],
            q: Quat9::IDENTITY
        }
    );
    assert_eq!(s.usable, vec![p(0, 0), p(650, 0), p(650, 900), p(0, 900)]);
    assert!(
        e.model()
            .table::<LibCopy<Part>>()
            .get(&copy_id(ITEM_ENCL))
            .is_some()
    );
    assert_eq!(
        rejected(e.execute(&from_part(vec![]))),
        "location J-E-2416 already has enclosure -UC1; a location has one enclosure"
    );
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before, "all gone, the copies included");
}

/// AS-5: a part without an enclosure section, a plate part not held, a function node that is not a function; a
/// location with a space and no enclosure reuses the space.
#[test]
fn as5_create_from_part_refusals_and_reuse() {
    let mut e = engine(model(&Spec {
        uc1: false,
        enclosure_copies: false,
        many: 0,
    }));
    let no_section = CreateEnclosureFromPart {
        part: copy_id(ITEM_MCB),
        ..from_part(vec![])
    };
    assert_eq!(
        rejected(e.execute(&no_section)),
        "MCB has no enclosure section; choose an enclosure part."
    );
    assert_eq!(
        rejected(e.execute(&from_part(vec![copy_json(enclosure_copy())]))),
        "the enclosure part names a plate part that the project does not hold; add it to copies"
    );
    assert_eq!(invalid(e.execute(&from_part(vec![]))).0, "/part");
    let wrong_function = CreateEnclosureFromPart {
        function: Some(LOC),
        ..from_part(vec![copy_json(enclosure_copy()), copy_json(plate_copy())])
    };
    assert_eq!(
        invalid(e.execute(&wrong_function)),
        (
            "/function".to_owned(),
            "node J-E-2416 is not a function".to_owned()
        )
    );
    let not_location = CreateEnclosureFromPart {
        location: FUNC_P01,
        ..from_part(vec![copy_json(enclosure_copy()), copy_json(plate_copy())])
    };
    assert_eq!(
        rejected(e.execute(&not_location)),
        "node P01 is not a location; a layout space lays out a location"
    );
    let space: Id<LayoutSpace> = created(
        e.execute(&CreateLayoutSpace {
            location: LOC,
            name: Some(ident("Cabinet")),
        })
        .unwrap(),
    );
    let enclosure: Id<Enclosure> = created(
        e.execute(&from_part(vec![
            copy_json(enclosure_copy()),
            copy_json(plate_copy()),
        ]))
        .unwrap(),
    );
    assert_eq!(rows::<LayoutSpace>(&e).len(), 1);
    assert_eq!(get(&e, enclosure).unwrap().space, space);
}

/// AS-6: each FT-840 edit changes its one member (whole records compared); a turned enclosure pose refused; missing
/// records at their pointers.
#[test]
fn as6_the_edits_change_one_member() {
    let mut f = fx();
    let space = get(&f.e, f.space).unwrap();
    f.e.execute(&EditLayoutSpace {
        space: f.space,
        name: Some(Some(ident("Cabinet"))),
    })
    .unwrap();
    let mut want = space.clone();
    want.name = Some(ident("Cabinet"));
    want.v = get(&f.e, f.space).unwrap().v;
    assert_eq!(get(&f.e, f.space).unwrap(), want);
    f.e.execute(&EditLayoutSpace {
        space: f.space,
        name: Some(None),
    })
    .unwrap();
    assert_eq!(get(&f.e, f.space).unwrap().name, None);
    assert_eq!(
        invalid(f.e.execute(&EditLayoutSpace {
            space: f.space,
            name: None
        })),
        (
            "/name".to_owned(),
            "give the name, or null to clear it".to_owned()
        )
    );
    assert_eq!(
        invalid(f.e.execute(&EditLayoutSpace {
            space: id(0xeeee),
            name: Some(None)
        })),
        (
            "/space".to_owned(),
            "the layout space is not in the project".to_owned()
        )
    );

    let enclosure = get(&f.e, f.enclosure).unwrap();
    let moved = Pose3 {
        t: [Nm(100 * MM), Nm(20 * MM), Nm(0)],
        q: Quat9::IDENTITY,
    };
    f.e.execute(&EditEnclosure {
        enclosure: f.enclosure,
        pose: moved,
    })
    .unwrap();
    let mut want = enclosure.clone();
    want.pose = moved;
    want.v = get(&f.e, f.enclosure).unwrap().v;
    assert_eq!(get(&f.e, f.enclosure).unwrap(), want);
    let turned = Pose3 {
        q: ew_derive::layout::Quarter::new(1).quat(),
        ..moved
    };
    assert_eq!(
        rejected(f.e.execute(&EditEnclosure {
            enclosure: f.enclosure,
            pose: turned
        })),
        "an enclosure's pose is a translation at this version; give it without a turn"
    );
    assert_eq!(
        invalid(f.e.execute(&EditEnclosure {
            enclosure: id(0xeeee),
            pose: moved
        })),
        (
            "/enclosure".to_owned(),
            "the enclosure is not in the project".to_owned()
        )
    );

    let surface = get(&f.e, f.surface).unwrap();
    f.e.execute(&EditSurface {
        surface: f.surface,
        pe_bond: true,
    })
    .unwrap();
    let mut want = surface.clone();
    want.pe_bond = true;
    want.v = get(&f.e, f.surface).unwrap().v;
    assert_eq!(get(&f.e, f.surface).unwrap(), want);
    assert_eq!(
        invalid(f.e.execute(&EditSurface {
            surface: id(0xeeee),
            pe_bond: true
        })),
        (
            "/surface".to_owned(),
            "the surface is not in the project".to_owned()
        )
    );
}

fn delete(targets: &[(KindName, ew_schema::AnyId)]) -> DeleteSelection {
    DeleteSelection {
        targets: targets
            .iter()
            .map(|(kind, id)| TargetRef { kind: *kind, id: *id })
            .collect(),
    }
}

fn placement_of(e: &Engine, device: Id<Device>) -> Id<Placement> {
    rows::<Placement>(e)
        .into_iter()
        .find(|p| p.device == device)
        .expect("a placement")
        .id
}

/// AS-7: `edit.delete` on the layout kinds with ruling 6's checks, each refusal whole; the model's cascades
/// otherwise.
#[test]
fn as7_edit_delete_on_layout_kinds() {
    let mut f = fx();
    f.e.execute(&place(k(1), f.rail1, 100)).unwrap();
    f.e.execute(&place(k(2), f.rail1, 200)).unwrap();
    let on_surface = PlaceDevice {
        carrier: PlaceOnParam::Surface(f.surface),
        pose: Pose3 {
            t: [Nm(300 * MM), Nm(100 * MM), Nm(0)],
            q: Quat9::IDENTITY,
        },
        ..place(k(3), f.rail1, 0)
    };
    f.e.execute(&on_surface).unwrap();
    let (p1, p2, p3) = (
        placement_of(&f.e, k(1)),
        placement_of(&f.e, k(2)),
        placement_of(&f.e, k(3)),
    );
    // fields.set refuses the layout kinds (they have no settable fields).
    let set = SetFields {
        kind: KindName::Placement,
        targets: vec![p1.cast()],
        path: "locked".into(),
        value: json!(true),
        copies: vec![],
    };
    assert_eq!(invalid(f.e.execute(&set)).0, "/path");
    let rail1 = (KindName::Carrier, f.rail1.cast());
    assert_eq!(
        rejected(f.e.execute(&delete(&[rail1]))),
        "Rail 1 holds 2 placements; use Delete carrier… to remove or rehome them."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[rail1, (KindName::Placement, p1.cast())]))),
        "Rail 1 holds 1 placement; use Delete carrier… to remove or rehome them."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[(KindName::Enclosure, f.enclosure.cast())]))),
        "Rail 1 holds 2 placements; use Delete carrier… to remove or rehome them."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[(KindName::LayoutSpace, f.space.cast())]))),
        "Rail 1 holds 2 placements; use Delete carrier… to remove or rehome them."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[
            (KindName::LayoutSpace, f.space.cast()),
            (KindName::Placement, p1.cast()),
            (KindName::Placement, p2.cast()),
        ]))),
        "The plate of -?UC1 holds 1 placement; remove them first."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[(KindName::Device, DEV_UC1.cast())]))),
        "-?UC1 carries an enclosure; delete the enclosure first."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[
            (KindName::Device, DEV_UC1.cast()),
            (KindName::Enclosure, f.enclosure.cast()),
        ]))),
        "-?UC1 carries an enclosure; delete the enclosure first."
    );
    assert_eq!(
        rejected(f.e.execute(&delete(&[(KindName::MountingSurface, f.surface.cast())]))),
        "The plate goes with its enclosure; delete the enclosure of -?UC1."
    );
    // A placement; an empty carrier; a carrier with all its placements.
    f.e.execute(&delete(&[(KindName::Placement, p3.cast())])).unwrap();
    assert!(get(&f.e, p3).is_none());
    f.e.execute(&delete(&[(KindName::Carrier, f.rail2.cast())]))
        .unwrap();
    assert!(get(&f.e, f.rail2).is_none());
    f.e.execute(&delete(&[
        rail1,
        (KindName::Placement, p1.cast()),
        (KindName::Placement, p2.cast()),
    ]))
    .unwrap();
    assert!(get(&f.e, f.rail1).is_none() && rows::<Placement>(&f.e).is_empty());
    // Now empty: the space goes with its enclosure, surface and carriers; -UC1 stays.
    f.e.execute(&delete(&[(KindName::LayoutSpace, f.space.cast())]))
        .unwrap();
    assert!(rows::<Enclosure>(&f.e).is_empty() && rows::<Carrier>(&f.e).is_empty());
    assert!(get(&f.e, DEV_UC1).is_some());
}

fn mv(placement: Id<Placement>, s: i64, part: Option<u128>, mode: Option<PlaceMode>) -> MovePlacement {
    MovePlacement {
        placement,
        carrier: None,
        pose: at(s),
        mode,
        part: part.map(copy_id),
    }
}

/// AS-8: `layout.move {part}`: another part use of the device (the resolver reads its extent with `mode`); a part the
/// device does not use, a duplicate key and an unfit part refused; without `part` the realised part stays.
#[test]
fn as8_move_realises_another_part() {
    let mut f = fx();
    let km = PlaceDevice {
        part: Some(copy_id(ITEM_MCB)),
        ..place(DEV_KM, f.rail1, 30)
    };
    let pl: Id<Placement> = created(f.e.execute(&km).unwrap()); // [21, 39]
    f.e.execute(&place(k(1), f.rail1, 50)).unwrap(); // [41, 59]
    f.e.execute(&mv(pl, 30, None, None)).unwrap();
    assert_eq!(get(&f.e, pl).unwrap().part, Some(PartRef::new(copy_id(ITEM_MCB))));
    // 40 mm wide from 10 mm: K1 is pushed to start at 50 mm.
    f.e.execute(&mv(pl, 30, Some(ITEM_WIDE), Some(PlaceMode::Push)))
        .unwrap();
    assert_eq!(
        get(&f.e, pl).unwrap().part,
        Some(PartRef::new(copy_id(ITEM_WIDE)))
    );
    assert_eq!((clip10(&f.e, DEV_KM), clip10(&f.e, k(1))), (300, 590));
    assert_eq!(
        invalid(f.e.execute(&mv(pl, 30, Some(ITEM_PAD), None))),
        ("/part".to_owned(), "-?KM1 does not use this part".to_owned())
    );
    assert_eq!(
        rejected(f.e.execute(&mv(pl, 30, Some(ITEM_TS32), None))),
        "-?KM1 (TS 32) does not fit a TS 35×7.5 rail"
    );
    let second = PlaceDevice {
        part: Some(copy_id(ITEM_MCB)),
        ..place(DEV_KM, f.rail2, 100)
    };
    f.e.execute(&second).unwrap();
    assert_eq!(
        rejected(f.e.execute(&mv(pl, 30, Some(ITEM_MCB), None))),
        "-?KM1 is already placed with this part and function; move its placement instead"
    );
}

/// Cycle-42 review part 2, finding 1: a member that joins the assembly after the preview (K3 attached to A1 at
/// position 4) changes the preview's reads (the `device_children` index of A1), so the commit re-runs the command
/// and places the new member too, instead of applying the stale prepared ops.
#[test]
fn finding1_a_concurrent_new_member_reruns_place_assembly() {
    let mut f = fx();
    let preview = f.e.preview(&assembly(DEV_A1, f.rail1, Some(20), None)).unwrap();
    let base = preview.base_rev();
    f.e.execute(&AttachChildren {
        parent: DEV_A1,
        children: vec![ChildSpec {
            device: k(3),
            position: Some(4),
        }],
    })
    .unwrap();
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the assembly read A1's member set"
    );
    f.e.commit(preview).unwrap();
    assert!(placed(&f.e, k(3)), "the re-run placed the new member -?K3");
    for d in [DEV_A1, DEV_M1, DEV_M2, DEV_M3] {
        assert!(placed(&f.e, d), "{d} is placed");
    }
}

/// Cycle-42 review part 2, finding 1: a device of the space that becomes unplaced after the preview (A1, whose
/// placement is removed) changes the default list's reads (the placement table), so the commit re-runs the fill and
/// places it, first in DT order with its members.
#[test]
fn finding1_a_concurrent_new_unplaced_device_reruns_auto_fill() {
    let mut f = fx();
    f.e.execute(&place(DEV_A1, f.rail2, 100)).unwrap();
    let a1 = placement_of(&f.e, DEV_A1);
    let preview = f.e.preview(&fill(f.rail1, None, FillOrder::Dt)).unwrap();
    let base = preview.base_rev();
    f.e.execute(&RemovePlacement { placement: a1 }).unwrap();
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the fill read the space's unplaced devices"
    );
    f.e.commit(preview).unwrap();
    assert!(placed(&f.e, DEV_A1), "the re-run placed -?A1");
    assert_eq!(
        clips(&f.e, &[DEV_A1, DEV_M3, DEV_M1, DEV_M2]),
        vec![9, 27, 45, 63]
    );
}

/// Cycle-42 review part 2, finding 1: a candidate re-tagged after the preview without a change to its own row (K4's
/// page renumbered below K5's, so its page-based tag sorts first) changes the sort key's reads (the page table), so
/// the commit re-runs the fill in the new DT order.
#[test]
fn finding1_a_concurrent_re_tag_reruns_auto_fill() {
    let mut f = fx();
    let tags = |e: &Engine| {
        let q = Q::new(e.model());
        [k(4), k(5)].map(|d| {
            q.try_get::<ew_derive::DeviceTag>(d)
                .ok()
                .and_then(|t| t.as_ref().as_ref().map(|t| t.full.clone()))
                .unwrap_or_default()
        })
    };
    assert_eq!(
        tags(&f.e),
        ["+J-E-2416-102K4".to_owned(), "+J-E-2416-101K5".to_owned()]
    );
    let preview =
        f.e.preview(&fill(f.rail1, Some(vec![k(4), k(5)]), FillOrder::Dt))
            .unwrap();
    let base = preview.base_rev();
    let renumber =
        f.e.preview(&RenumberPages {
            scope: PageScope::Pages(vec![PAGE_102]),
            start: 100,
            step: 1,
            keep_gaps: false,
        })
        .unwrap();
    f.e.commit(renumber).unwrap();
    assert_eq!(
        tags(&f.e),
        ["+J-E-2416-100K4".to_owned(), "+J-E-2416-101K5".to_owned()]
    );
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the fill read the pages behind the tags"
    );
    f.e.commit(preview).unwrap();
    assert_eq!(clips(&f.e, &[k(4), k(5)]), vec![9, 27], "-100K4 before -101K5");
}
