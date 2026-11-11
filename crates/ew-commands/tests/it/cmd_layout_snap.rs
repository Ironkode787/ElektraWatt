// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `layout.place` and `layout.move` with `mode` at the command layer, on a fixture rail.
//! Free, push and swap with every refusal text whole, one undo step per drop, clearance pads, the read set
//! that makes a neighbour's concurrent edit conflict, and the property that the
//! resolver a tool runs for its ghost gives exactly the committed poses. Golden A's numbers
//! are `ew-api`'s `tests/it/golden_rail_snap.rs`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddCarrier, CarrierKindParam, CarrierParamsParam, LockPlacement, MovePlacement, PlaceDevice, PlaceMode,
    PlaceOnParam,
};
use ew_commands::{CmdError, Engine, Origin};
use ew_derive::Q;
use ew_derive::layout::{
    Quarter, RailItem, RailOccupancy, Resolution, SnapMode, bracket_part, rail_extent, rail_items, resolve,
};
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{
    Carrier, Clearance, Device, LibCopy, LibDocument, MechData, MountKind, MountingSurface, Part, PartUse,
    Placement, Record,
};
use ew_units::{Nm, Pose3, Quat9};
use proptest::prelude::*;

const LIB_S: u128 = 0xd100;
const ITEM_ENCL: u128 = 0xd101;
const ITEM_PLATE: u128 = 0xd102;
const ITEM_RAIL: u128 = 0xd103;
const ITEM_MCB: u128 = 0xd104; // TS 35, 18 × 90 mm, centre clip
const ITEM_WIDE: u128 = 0xd105; // TS 35, 45 × 90 mm
const ITEM_PAD: u128 = 0xd106; // TS 35, 18 × 90 mm, clearance 5 mm left and right
const ITEM_LONG: u128 = 0xd107; // TS 35, 300 × 90 mm
const ITEM_LEFT: u128 = 0xd108; // TS 35, 18 × 90 mm, clearance 8 mm left only

const DEV_UC1: Id<Device> = id(0xa00);
const DEV_PLATE: Id<Device> = id(0xa01);
/// K1 … K8: 18 mm devices.
const fn k(n: u128) -> Id<Device> {
    id(0xa10 + n)
}
const DEV_W1: Id<Device> = id(0xa20);
const DEV_C1: Id<Device> = id(0xa21);
const DEV_C2: Id<Device> = id(0xa22);
const DEV_B1: Id<Device> = id(0xa23);
const DEV_A1: Id<Device> = id(0xa24);

fn copy_id(item: u128) -> Id<Part> {
    lib_part_id(LIB_S, item)
}

fn mech(w: i64, clearance: Option<(i64, i64)>) -> MechData {
    MechData {
        width: Some(Nm(w * MM)),
        height: Some(Nm(90 * MM)),
        mount_kind: Some(MountKind::Ts35x7_5),
        clearance: clearance.map(|(l, r)| Clearance {
            left: Nm(l * MM),
            right: Nm(r * MM),
            ..Clearance::default()
        }),
        ..MechData::default()
    }
}

fn copies() -> Vec<LibCopy<Part>> {
    let encl = enclosure_part(LIB_S, ITEM_ENCL, ITEM_PLATE, "ENCL");
    let plate = plate_part(LIB_S, ITEM_PLATE, "PLATE");
    let rail = rail_part(LIB_S, ITEM_RAIL, MountKind::Ts35x7_5);
    let device = |item, number, m| {
        let mut c = lib_part(LIB_S, item, number);
        c.item.mech = Some(m);
        c
    };
    vec![
        encl,
        plate,
        rail,
        device(ITEM_MCB, "MCB", mech(18, None)),
        device(ITEM_WIDE, "WIDE", mech(45, None)),
        device(ITEM_PAD, "PAD", mech(18, Some((5, 5)))),
        device(ITEM_LONG, "LONG", mech(300, None)),
        device(ITEM_LEFT, "LEFT", mech(18, Some((8, 0)))),
    ]
}

fn uses(item: u128) -> Vec<PartUse> {
    part_uses(LIB_S, &[item])
}

fn model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut uc1 = device(DEV_UC1, "UC", 1);
    uc1.parts = uses(ITEM_ENCL);
    let mut plate = device(DEV_PLATE, "UC", 2);
    plate.parent = Some(DEV_UC1);
    plate.parts = uses(ITEM_PLATE);
    records.extend([uc1, plate].map(Record::Device));
    for n in 1..=8u32 {
        let mut d = device(k(u128::from(n)), "K", n);
        d.parts = uses(ITEM_MCB);
        records.push(Record::Device(d));
    }
    for (dev, letter, item) in [
        (DEV_W1, "W", ITEM_WIDE),
        (DEV_C1, "C", ITEM_PAD),
        (DEV_C2, "C", ITEM_PAD),
        (DEV_B1, "B", ITEM_LONG),
        (DEV_A1, "A", ITEM_LEFT),
    ] {
        let n = if dev == DEV_C2 { 2 } else { 1 };
        let mut d = device(dev, letter, n);
        d.parts = uses(item);
        records.push(Record::Device(d));
    }
    let mut lib = library();
    lib.extend(copies().into_iter().map(LibDocument::Part));
    Model::from_records(p, records, lib).expect("valid")
}

/// A rail pose at clip point `s` (in tenths of a millimetre), turned `k` quarter turns.
fn at10(s: i64, k: u8) -> Pose3 {
    Pose3 {
        t: [Nm(s * MM / 10), Nm(0), Nm(0)],
        q: Quarter::new(k).quat(),
    }
}

fn at(s: i64) -> Pose3 {
    at10(s * 10, 0)
}

/// A rail (L = 600 mm) at height `y` mm.
fn rail_cmd(surface: Id<MountingSurface>, y: i64) -> AddCarrier {
    AddCarrier {
        surface,
        kind: CarrierKindParam::Rail,
        part: Some(copy_id(ITEM_RAIL)),
        path: vec![p(20, y), p(620, y)],
        params: CarrierParamsParam::Rail {
            profile: MountKind::Ts35x7_5,
        },
        pe_bond: None,
        copies: vec![],
    }
}

struct Fx {
    e: Engine,
    surface: Id<MountingSurface>,
    /// Rail 1 (top, y = 800 mm) and rail 2 (y = 600 mm).
    rail1: Id<Carrier>,
    rail2: Id<Carrier>,
}

fn fx() -> Fx {
    let mut e = engine(model());
    let (_, _, surface) = space_with_enclosure(&mut e, DEV_UC1, None);
    // Rail 2 is added first: its label comes from its place (top to bottom), not from its ID or its order.
    let rail2 = made(e.execute(&rail_cmd(surface, 600)).unwrap());
    let rail1 = made(e.execute(&rail_cmd(surface, 800)).unwrap());
    Fx {
        e,
        surface,
        rail1,
        rail2,
    }
}

fn place(device: Id<Device>, on: PlaceOnParam, pose: Pose3, mode: Option<PlaceMode>) -> PlaceDevice {
    PlaceDevice {
        device,
        part: None,
        function: None,
        carrier: on,
        pose,
        mode,
    }
}

fn mv(placement: Id<Placement>, pose: Pose3, mode: Option<PlaceMode>) -> MovePlacement {
    MovePlacement {
        placement,
        carrier: None,
        pose,
        mode,
        part: None,
    }
}

/// The clip point of a placement in tenths of a millimetre.
fn clip10(e: &Engine, pl: Id<Placement>) -> i64 {
    e.model()
        .table::<Placement>()
        .get(&pl)
        .expect("the placement")
        .pose
        .t[0]
        .0
        * 10
        / MM
}

impl Fx {
    /// Places a device on rail 1 at clip `s` mm without a mode.
    fn put(&mut self, device: Id<Device>, s: i64) -> Id<Placement> {
        made(
            self.e
                .execute(&place(device, PlaceOnParam::Carrier(self.rail1), at(s), None))
                .unwrap(),
        )
    }
}

/// SN-2 (free): a device overlapping its neighbours goes to the nearest gap where it fits (nothing else moves); a
/// position that fits is kept; no gap wide enough is refused naming the rail by its label and the device by its tag.
#[test]
fn free_takes_the_nearest_gap() {
    let mut f = fx();
    let k1 = f.put(k(1), 100); // [91, 109]
    let k2 = f.put(k(2), 118); // [109, 127]
    let cursor = f.e.history().cursor();
    // K3 at clip 112 mm (left edge 103): the left gap ends at 73 mm (30 away), the right one starts at 127 mm (24 away).
    let k3: Id<Placement> = made(
        f.e.execute(&place(
            k(3),
            PlaceOnParam::Carrier(f.rail1),
            at(112),
            Some(PlaceMode::Free),
        ))
        .unwrap(),
    );
    assert_eq!(clip10(&f.e, k3), 1360);
    assert_eq!((clip10(&f.e, k1), clip10(&f.e, k2)), (1000, 1180));
    assert_eq!(f.e.history().cursor(), cursor + 1);
    let k4: Id<Placement> = made(
        f.e.execute(&place(
            k(4),
            PlaceOnParam::Carrier(f.rail1),
            at(300),
            Some(PlaceMode::Free),
        ))
        .unwrap(),
    );
    assert_eq!(clip10(&f.e, k4), 3000, "a place that fits is kept");
    // Gaps of 91, 146 and 291 mm: the 300 mm device fits none.
    let refused = reason(f.e.execute(&place(
        DEV_B1,
        PlaceOnParam::Carrier(f.rail1),
        at(400),
        Some(PlaceMode::Free),
    )));
    assert_eq!(refused, "No gap on rail 1 is wide enough for -?B1 (300 mm).");
    // Rail 2 is named by its place, below rail 1.
    for (n, s) in [(5, 100), (6, 250), (7, 400), (8, 550)] {
        f.e.execute(&place(k(n), PlaceOnParam::Carrier(f.rail2), at(s), None))
            .unwrap();
    }
    let refused = reason(f.e.execute(&place(
        DEV_B1,
        PlaceOnParam::Carrier(f.rail2),
        at(300),
        Some(PlaceMode::Free),
    )));
    assert_eq!(refused, "No gap on rail 2 is wide enough for -?B1 (300 mm).");
}

/// SN-3 (push): a device dropped between two inserts there and moves the ones to its right in the same step (one undo
/// restores all); a locked neighbour that would move refuses; a neighbour pushed past the rail end refuses.
#[test]
fn push_inserts_and_moves_the_right_neighbours() {
    let mut f = fx();
    let k1 = f.put(k(1), 100); // [91, 109]
    let k2 = f.put(k(2), 118); // [109, 127]
    let k3 = f.put(k(3), 136); // [127, 145]
    let before = lines(f.e.model());
    // K4 at clip 118 mm: K2's centre is exactly under it, so K2 counts as right and moves.
    let k4: Id<Placement> = made(
        f.e.execute(&place(
            k(4),
            PlaceOnParam::Carrier(f.rail1),
            at(118),
            Some(PlaceMode::Push),
        ))
        .unwrap(),
    );
    assert_eq!(
        [k1, k4, k2, k3].map(|p| clip10(&f.e, p)),
        [1000, 1180, 1360, 1540]
    );
    f.e.undo(&Origin::Gui).unwrap();
    assert_eq!(lines(f.e.model()), before, "one undo step restores every pose");

    // Locked placements other than the named one are never moved.
    f.e.execute(&LockPlacement {
        placement: k3,
        locked: true,
    })
    .unwrap();
    let refused = reason(f.e.execute(&place(
        k(4),
        PlaceOnParam::Carrier(f.rail1),
        at(118),
        Some(PlaceMode::Push),
    )));
    assert_eq!(refused, "Pushing would move -?K3, which is locked.");
    f.e.undo(&Origin::Gui).unwrap();

    // K5 near the end [581, 599]; the 300 mm device at clip 450 mm ends at 600 mm and pushes K5 18 mm past the end.
    let k5 = f.put(k(5), 590);
    let refused = reason(f.e.execute(&place(
        DEV_B1,
        PlaceOnParam::Carrier(f.rail1),
        at(450),
        Some(PlaceMode::Push),
    )));
    assert_eq!(refused, "Pushing would move -?K5 past the rail end by 18 mm.");
    // Alone past the end, the device itself is named.
    f.e.execute(&ew_commands::commands::RemovePlacement { placement: k5 })
        .unwrap();
    let refused = reason(f.e.execute(&place(
        DEV_B1,
        PlaceOnParam::Carrier(f.rail1),
        at(500),
        Some(PlaceMode::Push),
    )));
    assert_eq!(refused, "-?B1 would pass the rail end by 50 mm.");
}

/// `layout.move` with no mode, or in free mode, moves the named placement whatever
/// its lock … In push or swap, a locked named placement is refused with the whole text 'Unlock F55 to push or swap
/// it.'
#[test]
fn a_locked_placement_moves_by_itself_only() {
    let mut f = fx();
    let k1 = f.put(k(1), 100);
    let k2 = f.put(k(2), 300);
    f.e.execute(&LockPlacement {
        placement: k1,
        locked: true,
    })
    .unwrap();
    for mode in [PlaceMode::Push, PlaceMode::Swap] {
        let refused = reason(f.e.execute(&mv(k1, at(300), Some(mode))));
        assert_eq!(refused, "Unlock -?K1 to push or swap it.");
    }
    f.e.execute(&mv(k1, at(200), Some(PlaceMode::Free))).unwrap();
    assert_eq!(clip10(&f.e, k1), 2000);
    f.e.execute(&mv(k1, at(250), None)).unwrap();
    assert_eq!(clip10(&f.e, k1), 2500);
    assert_eq!(clip10(&f.e, k2), 3000, "nothing else moved");
}

/// SN-4 (swap): a placement dropped onto another exchanges with it, the segment between re-laid and nothing else
/// moved; an empty spot, another rail, a device not placed yet and a locked target refuse.
#[test]
fn swap_exchanges_within_one_rail() {
    let mut f = fx();
    let k1 = f.put(k(1), 100); // [91, 109]
    let k2 = f.put(k(2), 118); // [109, 127]
    let w1 = f.put(DEV_W1, 200); // [177.5, 222.5]
    let k5 = f.put(k(5), 400);
    // K1 dropped with its centre on W1's: the segment K1, K2, W1 becomes W1, K2, K1 from 91 mm.
    let cursor = f.e.history().cursor();
    f.e.execute(&mv(k1, at(200), Some(PlaceMode::Swap))).unwrap();
    assert_eq!(
        [w1, k2, k1, k5].map(|p| clip10(&f.e, p)),
        [1135, 1450, 2135, 4000],
        "W1 [91, 136], K2 keeps its 0 mm gap [136, 154], K1 keeps the 50.5 mm gap [204.5, 222.5]"
    );
    assert_eq!(f.e.history().cursor(), cursor + 1);
    let refused = reason(f.e.execute(&mv(k1, at(300), Some(PlaceMode::Swap))));
    assert_eq!(refused, "Drop onto a placement to swap with it.");
    let across = MovePlacement {
        placement: k1,
        carrier: Some(PlaceOnParam::Carrier(f.rail2)),
        pose: at(100),
        mode: Some(PlaceMode::Swap),
        part: None,
    };
    let swap_text = "Swap works within one rail; use push or free to place it there.";
    assert_eq!(reason(f.e.execute(&across)), swap_text);
    let new = place(
        k(6),
        PlaceOnParam::Carrier(f.rail1),
        at(400),
        Some(PlaceMode::Swap),
    );
    assert_eq!(reason(f.e.execute(&new)), swap_text);
    f.e.execute(&LockPlacement {
        placement: k5,
        locked: true,
    })
    .unwrap();
    let refused = reason(f.e.execute(&mv(k2, at(400), Some(PlaceMode::Swap))));
    assert_eq!(refused, "Swapping would move -?K5, which is locked.");
}

/// D4: on a surface only free is accepted (the explicit pose); push and swap are refused. On a duct the pose check
/// refuses first, as without a mode.
#[test]
fn modes_on_a_surface() {
    let mut f = fx();
    let pose = Pose3 {
        t: [Nm(100 * MM), Nm(100 * MM), Nm(0)],
        q: Quat9::IDENTITY,
    };
    for mode in [PlaceMode::Push, PlaceMode::Swap] {
        let refused = reason(f.e.execute(&place(k(1), PlaceOnParam::Surface(f.surface), pose, Some(mode))));
        assert_eq!(refused, "Push and swap work on a rail.");
    }
    let pl: Id<Placement> = made(
        f.e.execute(&place(
            k(1),
            PlaceOnParam::Surface(f.surface),
            pose,
            Some(PlaceMode::Free),
        ))
        .unwrap(),
    );
    assert_eq!(f.e.model().table::<Placement>().get(&pl).unwrap().pose, pose);
}

/// SN-5 (clearance): two parts with 5 mm pads keep a 10 mm gap in free mode; a flipped placement's pads swap; without a
/// mode the pads are not applied (the pose as given).
#[test]
fn clearance_pads_space_the_snapping() {
    let mut f = fx();
    let c1 = f.put(DEV_C1, 300); // [291, 309]
    let c2: Id<Placement> = made(
        f.e.execute(&place(
            DEV_C2,
            PlaceOnParam::Carrier(f.rail1),
            at(318),
            Some(PlaceMode::Free),
        ))
        .unwrap(),
    );
    assert_eq!(clip10(&f.e, c2), 3280, "10 mm after C1's end: 5 mm pad each");
    f.e.execute(&mv(c2, at(318), None)).unwrap();
    assert_eq!(clip10(&f.e, c2), 3180, "without a mode the pose is as given");
    let a1 = f.put(DEV_A1, 100);
    let pads = |e: &Engine| {
        let q = Q::new(e.model());
        let occ = q.get::<RailOccupancy>(f.rail1);
        let a = occ.iter().find(|o| o.placement == a1).unwrap().pad;
        let c = occ.iter().find(|o| o.placement == c1).unwrap().pad;
        (a, c)
    };
    assert_eq!(
        pads(&f.e),
        ([8 * MM as i128, 0], [5 * MM as i128, 5 * MM as i128])
    );
    f.e.execute(&mv(a1, at10(1000, 2), None)).unwrap();
    assert_eq!(pads(&f.e).0, [0, 8 * MM as i128], "flipped, the pads swap");
}

/// The command also reads, through `tx`, the carrier, every
/// placement on it, and the strip and bracket rows the items came from, so that a concurrent edit of a neighbour on
/// that rail is detected at merge. A push previewed before a neighbour moved under it re-runs at commit
/// instead of applying the stale poses.
#[test]
fn a_neighbours_concurrent_edit_conflicts_at_merge() {
    let mut f = fx();
    let k1 = f.put(k(1), 100); // [91, 109]
    let k2 = f.put(k(2), 136); // [127, 145]
    let push = place(
        k(3),
        PlaceOnParam::Carrier(f.rail1),
        at(118),
        Some(PlaceMode::Push),
    );
    let preview = f.e.preview(&push).unwrap();
    let base = preview.base_rev();
    // Meanwhile K1, left of the drop and not written by the push, moves to clip 112 mm [103, 121].
    f.e.execute(&mv(k1, at(112), None)).unwrap();
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the push read K1's row"
    );
    let k3: Id<Placement> = made(f.e.commit(preview).unwrap());
    // Re-run on the current rail: K3 starts after K1's new end (121 mm), K2 is pushed to 139 mm.
    assert_eq!([k1, k3, k2].map(|p| clip10(&f.e, p)), [1120, 1300, 1480]);
    // An edit on another rail does not conflict.
    let other = place(
        k(4),
        PlaceOnParam::Carrier(f.rail1),
        at(400),
        Some(PlaceMode::Free),
    );
    let preview = f.e.preview(&other).unwrap();
    let base = preview.base_rev();
    f.e.execute(&place(k(5), PlaceOnParam::Carrier(f.rail2), at(100), None))
        .unwrap();
    assert!(preview.read_set().unchanged_since(f.e.model(), base));
}

/// `note_rail_reads` records the rail's placements index, so a concurrent insert onto the same rail, which
/// writes no row the push read, still conflicts at merge, and the push re-runs on the current rail.
#[test]
fn a_concurrent_insert_onto_the_rail_conflicts_at_merge() {
    let mut f = fx();
    let k1 = f.put(k(1), 100); // [91, 109]
    let k2 = f.put(k(2), 136); // [127, 145]
    let push = place(
        k(3),
        PlaceOnParam::Carrier(f.rail1),
        at(118),
        Some(PlaceMode::Push),
    );
    let preview = f.e.preview(&push).unwrap();
    let base = preview.base_rev();
    // Meanwhile K4 is placed far right on the same rail, clear of every item the push moves.
    let k4 = f.put(k(4), 400);
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the push read the rail's placements"
    );
    let k3: Id<Placement> = made(f.e.commit(preview).unwrap());
    assert_eq!(
        [k1, k3, k2, k4].map(|p| clip10(&f.e, p)),
        [1000, 1180, 1360, 4000]
    );
}

/// `note_rail_reads` records the neighbours' part copies, so a concurrent clearance edit of a neighbour's part
/// copy (a `library.update` of its 5 mm pads to 9 mm, which changes no placement or device row) conflicts at merge.
#[test]
fn a_concurrent_clearance_edit_of_a_neighbours_part_copy_conflicts_at_merge() {
    let mut f = fx();
    f.put(DEV_C1, 100); // PAD: [91, 109], 5 mm pads
    f.put(k(2), 136);
    let push = place(
        k(3),
        PlaceOnParam::Carrier(f.rail1),
        at(118),
        Some(PlaceMode::Push),
    );
    let preview = f.e.preview(&push).unwrap();
    let base = preview.base_rev();
    let mut pad = copies()
        .into_iter()
        .find(|c| c.id == copy_id(ITEM_PAD))
        .expect("the pad copy");
    pad.version = "1.1.0".into();
    pad.item.mech = Some(mech(18, Some((9, 9))));
    let mut v = serde_json::to_value(LibDocument::Part(pad)).expect("a library document");
    let hash = ew_commands::commands::library_copy_hash(
        &LibDocument::from_value(v.clone()).expect("a library document"),
    )
    .expect("a known kind");
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    v["sha256"] = serde_json::json!(format!("sha256:{hex}"));
    let update = ew_commands::AnyCommand {
        name: "library.update".into(),
        params: serde_json::json!({ "copies": [v] }),
    };
    let update = f.e.preview(&update).expect("library.update previews");
    f.e.commit(update).expect("library.update");
    assert!(
        !preview.read_set().unchanged_since(f.e.model(), base),
        "the push read the neighbour's part copy"
    );
}

/// SN-7: without `mode` the JSON form is unchanged (no `mode` member); with it, `mode` round-trips.
#[test]
fn mode_is_an_optional_member() {
    let rail: Id<Carrier> = id(0xb2);
    let plain = serde_json::to_value(place(k(1), PlaceOnParam::Carrier(rail), at(96), None)).unwrap();
    assert!(plain.get("mode").is_none());
    let pushed = place(k(1), PlaceOnParam::Carrier(rail), at(96), Some(PlaceMode::Push));
    let v = serde_json::to_value(&pushed).unwrap();
    assert_eq!(v["mode"], serde_json::json!("push"));
    assert_eq!(serde_json::from_value::<PlaceDevice>(v).unwrap(), pushed);
    let moved: MovePlacement = serde_json::from_value(serde_json::json!({
        "placement": id::<Placement>(0xb3).to_string(),
        "pose": {"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]},
        "mode": "swap"
    }))
    .unwrap();
    assert_eq!(moved.mode, Some(PlaceMode::Swap));
}

// ---------------------------------------------------------------------------------------------------------
// SN-7: the ghost equals the commit. Bounds: rail 1 of 600 mm, up to 6 devices (18 or 45 mm) placed at random clip
// points first (no mode), then one drop (a new device placed, or a placed one moved, at a random clip point in
// [0, 600] mm, either turn) in a random mode; 48 cases.

/// The tool's ghost: what part 3's layout tool computes from the same public functions before the commit.
fn ghost(
    model: &Model,
    rail: Id<Carrier>,
    device: Id<Device>,
    placement: Option<Id<Placement>>,
    pose: &Pose3,
    mode: SnapMode,
) -> Result<(Resolution, i128, Vec<RailItem>), ew_derive::layout::SnapError> {
    let q = Q::new(model);
    let items = rail_items(&q, rail);
    let dev = q.devices().get(device).unwrap().clone();
    let stored = placement.and_then(|p| q.table::<Placement>().get(p).and_then(|r| r.part.clone()));
    let part = stored.or_else(|| ew_derive::layout::primary_part(&q, &dev));
    let flipped = Quarter::of(pose.q) == Some(Quarter::HALF);
    let ext = rail_extent(&q, &dev, part.as_ref(), flipped);
    let moved = RailItem {
        s0: 0,
        s1: ext.width,
        strip: ext.strip,
        w: if ext.strip {
            bracket_part(&q, &dev).map(|(_, w)| w)
        } else {
            None
        },
        pad: ext.pad,
        locked: placement
            .and_then(|p| q.table::<Placement>().get(p).map(|r| r.locked))
            .unwrap_or(false),
        placement,
    };
    let s_left = i128::from(pose.t[0].0) - ext.clip;
    resolve(&items, 600 * i128::from(MM), moved, s_left, mode).map(|r| (r, ext.width, items))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn the_ghost_equals_the_commit(
        setup in prop::collection::vec((0i64..=600, any::<bool>(), any::<bool>()), 0..=6),
        drop_s in 0i64..=600,
        flip in any::<bool>(),
        mode in 0usize..3,
        pick in 0usize..8,
        lock in any::<bool>(),
    ) {
        let mut f = fx();
        let mut placed = Vec::new();
        for (n, (s, wide, locked)) in setup.iter().enumerate() {
            let device = if *wide && n == 0 { DEV_W1 } else { k(n as u128 + 1) };
            let pl: Id<Placement> = made(
                f.e.execute(&place(device, PlaceOnParam::Carrier(f.rail1), at(*s), None)).unwrap(),
            );
            if *locked && lock {
                f.e.execute(&LockPlacement { placement: pl, locked: true }).unwrap();
            }
            placed.push((pl, device));
        }
        let (snap, param) = [
            (SnapMode::Free, PlaceMode::Free),
            (SnapMode::Push, PlaceMode::Push),
            (SnapMode::Swap, PlaceMode::Swap),
        ][mode];
        let pose = at10(drop_s * 10, if flip { 2 } else { 0 });
        let moving = (!placed.is_empty() && pick < placed.len()).then(|| placed[pick]);
        let (device, placement) = match moving {
            Some((pl, d)) => (d, Some(pl)),
            None => (k(8), None),
        };
        let before = f.e.model().clone();
        let expected = ghost(&before, f.rail1, device, placement, &pose, snap);
        let result = match placement {
            Some(pl) => f.e.execute(&mv(pl, pose, Some(param))).map(|_| pl),
            None => f.e.execute(&place(device, PlaceOnParam::Carrier(f.rail1), pose, Some(param))).map(made),
        };
        match (expected, result) {
            (Ok((r, width, items)), Ok(pl)) => {
                let q = Q::new(f.e.model());
                let occ = q.get::<RailOccupancy>(f.rail1);
                let s0 = |p: Id<Placement>| occ.iter().find(|o| o.placement == p).map(|o| (o.s0, o.s1));
                prop_assert_eq!(s0(pl), Some((r.s_left, r.s_left + width)));
                for item in &items {
                    let Some(p) = item.placement else { continue };
                    if Some(p) == placement { continue; }
                    let want = r.moved.iter().find(|(m, _)| *m == p).map_or(item.s0, |(_, s)| *s);
                    prop_assert_eq!(s0(p).map(|x| x.0), Some(want));
                }
            }
            (Err(_), Err(CmdError::Rejected { .. })) => {
                prop_assert_eq!(lines(f.e.model()), lines(&before));
            }
            (Err(_), Err(other)) => prop_assert!(false, "unexpected error {other:?}"),
            (Ok(_), Err(e)) => prop_assert!(false, "the ghost resolved but the command refused: {e:?}"),
            (Err(e), Ok(_)) => prop_assert!(false, "the ghost refused ({e:?}) but the command committed"),
        }
    }
}
