// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout queries over Golden A's test geometry (WP-S4.01 part 2, LQ-1 to LQ-5; doc 20 §13, U-50):
//!
//! - `lq1_golden_a_holds_the_layout`: one space for +J-E-2416, the enclosure -UC1 with one plate surface, the eight
//!   carriers (rails 1.71 m, ducts 3.17 m), the twenty placements; `Session::layout_spaces`.
//! - `lq2_unplaced_is_the_five_devices_each_case_asserted`: `unplaced` is F55, E55, E56, S56, -201U9 and every other
//!   device of the brief is out for its D8 case; a second space lists only its own location's devices.
//! - `lq3_footprints_in_the_surface_frame`: the footprints of -102G1 and -X1031 (orchestrator ruling 1), the flip, a
//!   screw part turned 90° on the surface, the unknown square.
//! - `lq4_a_wider_part_widens_the_footprint_and_overlaps`: `part.assign` of a 27 mm part with `rail_offset` (9, 42.5).
//! - `lq5_labels_follow_the_tag_and_no_layout_file_changes`: the two label modes; T55 → T57 on a saved copy of the
//!   fixture changes its label and no file under `layout/`.
//! - `the_layout_queries_memoise_as_they_run_cold`: memo against cold (and the engine's oracle) over layout edits.

// The save test copies the fixture into a clock-named scratch directory (`ew-testdir`).
#![allow(clippy::disallowed_methods)]

mod golden_common;

use std::collections::BTreeMap;
use std::path::Path;

use ew_api::{AnyCommand, CmdError, MemoMode, Recovery, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::layout::{
    CarrierGeometry, Exempt, LabelMode, LayoutLabel, MountAs, Mountability, Mountable, PlacementGeometry,
    RailOccupancy, Unplaced,
};
use ew_import_tables::ids::record_id;
use ew_schema::{
    Carrier, CarrierKind, Device, Enclosure, LayoutSpace, LibCopy, LibDocument, MountingSurface, Page, Part,
    Placement, Record, Sha256, StructureNode,
};
use ew_store::StoreOptions;
use ew_units::{Nm, Pt2, Rect};
use serde_json::{Value, json};

const MM: i128 = 1_000_000;

/// A point in tenths of a millimetre (`58.6` mm is `586`).
fn p10(u: i128, v: i128) -> Pt2 {
    let nm = |x: i128| Nm(i64::try_from(x * MM / 10).expect("in range"));
    Pt2::new(nm(u), nm(v))
}

/// A rectangle in tenths of a millimetre.
fn rect10(a: (i128, i128), b: (i128, i128)) -> Rect {
    Rect::from_points(p10(a.0, a.1), p10(b.0, b.1))
}

fn session(memo: MemoMode) -> Session {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    Session::from_records(
        project.expect("the batch holds the project"),
        records,
        batch.lib.clone(),
        SessionOptions {
            memo,
            ids: Some(seeded_ids(40)),
            ..SessionOptions::default()
        },
    )
    .expect("the golden batch is a valid model")
}

fn dev(key: &str) -> Id<Device> {
    record_id("devices", key)
}

fn terminal(key: &str) -> Id<Device> {
    record_id("terminals", key)
}

fn placement(key: &str) -> Id<Placement> {
    record_id("placements", key)
}

fn carrier(key: &str) -> Id<Carrier> {
    record_id("carriers", key)
}

fn space() -> Id<LayoutSpace> {
    record_id("layout_spaces", "+J-E-2416")
}

fn surface() -> Id<MountingSurface> {
    record_id("mounting_surfaces", "-UC1")
}

fn page(key: &str) -> Id<Page> {
    record_id("pages", key)
}

fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

/// Runs a command (a bulk one through preview and commit) and returns the receipt's primary record ID as JSON.
fn run(session: &mut Session, name: &str, params: Value) -> Option<Value> {
    let cmd = AnyCommand {
        name: name.into(),
        params: params.clone(),
    };
    let receipt = match session.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => session.preview(&cmd).and_then(|p| session.commit(p)),
        other => other,
    }
    .unwrap_or_else(|e| panic!("{name} {params}: {e:?}"));
    receipt
        .outcome
        .primary
        .map(|k| serde_json::to_value(k.1).expect("a key serialises"))
}

fn geometry(q: &Q, p: Id<Placement>) -> ew_derive::layout::PlacementGeom {
    q.get::<PlacementGeometry>(p)
        .as_ref()
        .clone()
        .unwrap_or_else(|| panic!("placement {p} has a geometry"))
}

/// The occupied interval of a placement on a rail, in tenths of a millimetre along the axis.
fn interval10(q: &Q, rail: Id<Carrier>, p: Id<Placement>) -> (i128, i128) {
    let occ = q.get::<RailOccupancy>(rail);
    let o = occ
        .iter()
        .find(|o| o.placement == p)
        .unwrap_or_else(|| panic!("placement {p} is on the rail"));
    (o.s0 * 10 / MM, o.s1 * 10 / MM)
}

fn quarter(k: u8) -> Value {
    serde_json::to_value(ew_derive::layout::Quarter::new(k).quat()).expect("a quaternion serialises")
}

#[test]
fn lq1_golden_a_holds_the_layout() {
    let s = session(MemoMode::Off);
    let q = s.query();
    let location: Id<StructureNode> = record_id("structure", "+J-E-2416");
    assert_eq!(s.layout_spaces(), vec![(space(), location)]);
    let enclosures: Vec<_> = q.table::<Enclosure>().iter().map(|(_, e)| e.device).collect();
    assert_eq!(enclosures, vec![dev("-UC1")]);
    let surfaces: Vec<_> = q
        .table::<MountingSurface>()
        .iter()
        .map(|(id, s)| (id, s.device))
        .collect();
    assert_eq!(surfaces, vec![(surface(), Some(dev("-UC1-plate")))]);
    let mut lengths: BTreeMap<&str, i128> = BTreeMap::new();
    for (id, c) in q.table::<Carrier>().iter() {
        let g = q.get::<CarrierGeometry>(id);
        let g = g.as_ref().as_ref().expect("a carrier geometry");
        assert_eq!(g.path_error, None, "{id}");
        let kind = if c.kind == CarrierKind::Rail {
            "rails"
        } else {
            "ducts"
        };
        *lengths.entry(kind).or_default() += g.length.expect("a length");
    }
    // doc 20 §13's test geometry: "Rails 1.71 m, ducts 3.17 m."
    assert_eq!(
        lengths,
        BTreeMap::from([("ducts", 3170 * MM), ("rails", 1710 * MM)])
    );
    assert_eq!(q.table::<Carrier>().iter().count(), 8);
    assert_eq!(q.table::<Placement>().iter().count(), 20);
}

#[test]
fn lq2_unplaced_is_the_five_devices_each_case_asserted() {
    let mut s = session(MemoMode::Off);
    let q = s.query();
    let unplaced = s.unplaced(space());
    // In natural order of the full tag: "=P01+J-E-2416-201U9" sorts before the frozen tags "…E55".
    assert_eq!(
        unplaced,
        vec![dev("-201U9"), dev("E55"), dev("E56"), dev("F55"), dev("S56")]
    );
    let m = |id: Id<Device>| q.get::<Mountable>(id).as_ref().clone();
    let part = |how: MountAs, id: Id<Device>| match m(id) {
        Mountability::Mountable { how: h, primary } => h == how && primary.is_some(),
        _ => false,
    };
    // Case 5: a part with a mount kind (TS 35: -201U9, F55; screw: E55).
    assert!(part(MountAs::Part, dev("-201U9")));
    assert!(part(MountAs::Part, dev("F55")));
    assert!(part(MountAs::Part, dev("E55")));
    // Case 9: no part, unknown footprint.
    for key in ["E56", "S56"] {
        assert_eq!(
            m(dev(key)),
            Mountability::Mountable {
                how: MountAs::NoPart,
                primary: None
            },
            "{key}"
        );
    }
    // Case 3: a strip with terminals is mountable and placed whole (it is placed, so not listed).
    assert_eq!(
        m(dev("-X1031")),
        Mountability::Mountable {
            how: MountAs::Strip,
            primary: None
        }
    );
    // Case 8: -XPE, the integral PE point; case 6: the DP plug -3X1 and the valves mount on their parent.
    assert_eq!(m(dev("-XPE")), Mountability::Exempt(Exempt::PePoint));
    assert_eq!(m(dev("-3X1")), Mountability::Exempt(Exempt::WithParent));
    for valve in ["-201U3-Q1", "-201U8-Q4", "-202U2-Q2"] {
        assert_eq!(m(dev(valve)), Mountability::Exempt(Exempt::WithParent), "{valve}");
    }
    // Case 4: the terminals and the cables.
    for t in ["-X1031:1", "-X1032:5"] {
        assert_eq!(m(terminal(t)), Mountability::Exempt(Exempt::Terminal), "{t}");
    }
    for cable in ["-3W1", "-106W1", "-106W2"] {
        assert_eq!(m(dev(cable)), Mountability::Exempt(Exempt::Cable), "{cable}");
    }
    // Case 2: the enclosure device and the plate device are the layout.
    assert_eq!(m(dev("-UC1")), Mountability::Exempt(Exempt::Layout));
    assert_eq!(m(dev("-UC1-plate")), Mountability::Exempt(Exempt::Layout));
    // Case 1: +FIELD has no layout space.
    for valve in ["-203Q1", "-203Q8"] {
        assert_eq!(m(dev(valve)), Mountability::NotConsidered, "{valve}");
        assert!(!unplaced.contains(&dev(valve)));
    }
    // FR-LAY-001: a space for +FIELD lists the field valves and nothing of +J-E-2416; the cabinet's list is unchanged.
    let field: Id<StructureNode> = record_id("structure", "+FIELD");
    let field_space: Id<LayoutSpace> = serde_json::from_value(
        run(&mut s, "layout.space.create", json!({"location": id_json(field)})).unwrap(),
    )
    .unwrap();
    let field_list = s.unplaced(field_space);
    assert!(field_list.contains(&dev("-203Q1")) && field_list.contains(&dev("-203Q8")));
    assert!(field_list.iter().all(|d| !unplaced.contains(d)));
    assert_eq!(s.unplaced(space()), unplaced);
}

/// Orchestrator ruling 1 (cycle 40): "LQ-3 footprints are in the surface frame: u = path[0].u + s". -102G1 on R1 is
/// (101, 675)–(171, 805) mm; -X1031 on R2 is (98.6, 444)–(202.6, 516) mm; the spec's (61, …)–(131, …) and (58.6, …)–
/// (162.6, …) are the same rectangles as `rail_occupancy` intervals in `s` (R1 and R2 start at u = 40 mm).
#[test]
fn lq3_footprints_in_the_surface_frame() {
    let mut s = session(MemoMode::Off);
    let q = s.query();
    let g1 = geometry(&q, placement("-102G1"));
    assert_eq!(g1.footprint, rect10((1010, 6750), (1710, 8050)));
    assert_eq!((g1.surface, g1.unknown), (surface(), false));
    assert_eq!(interval10(&q, carrier("R1"), placement("-102G1")), (610, 1310));
    let x1031 = geometry(&q, placement("-X1031"));
    assert_eq!(x1031.footprint, rect10((986, 4440), (2026, 5160)));
    assert_eq!(x1031.part, None, "a strip");
    assert_eq!(interval10(&q, carrier("R2"), placement("-X1031")), (586, 1626));

    // Flipping -102G1 (180° on the rail) gives the same rectangle: the clip point is its centre.
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-102G1")), "pose": {"t": [96_000_000, 0, 0], "q": quarter(2)}}),
    );
    let flipped = geometry(&s.query(), placement("-102G1"));
    assert_eq!(flipped.footprint, g1.footprint);
    assert_eq!(flipped.turn.k(), 2);

    // The screw part of E55 (50 × 120 mm) on the surface at (500, 100) mm: 0° spans W × H from its origin, 90° swaps
    // W and H about the origin.
    let pose = |k: u8| json!({"t": [500_000_000, 100_000_000, 0], "q": quarter(k)});
    let e55 = placement_id(run(
        &mut s,
        "layout.place",
        json!({"device": id_json(dev("E55")), "carrier": {"surface": id_json(surface())}, "pose": pose(0)}),
    ));
    assert_eq!(
        geometry(&s.query(), e55).footprint,
        rect10((5000, 1000), (5500, 2200))
    );
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(e55), "pose": pose(1)}),
    );
    assert_eq!(
        geometry(&s.query(), e55).footprint,
        rect10((3800, 1000), (5000, 1500))
    );

    // E56 has no part: the unknown 20 × 20 mm square about its clip point on R1 at s = 20 mm, (60, 740).
    let e56 = placement_id(run(
        &mut s,
        "layout.place",
        json!({"device": id_json(dev("E56")), "carrier": {"carrier": id_json(carrier("R1"))},
               "pose": {"t": [20_000_000, 0, 0], "q": quarter(0)}}),
    ));
    let g = geometry(&s.query(), e56);
    assert!(g.unknown);
    assert_eq!(g.footprint, rect10((500, 7300), (700, 7500)));
}

fn placement_id(v: Option<Value>) -> Id<Placement> {
    serde_json::from_value(v.expect("a primary record")).expect("a placement ID")
}

/// LQ-4: -102F1 (A9F78116, 18 mm, centre clip) at s = 158 mm gets a 27 mm test part whose `rail_offset` (9, 42.5) mm is
/// the 18 mm part's clip point (D7): the footprint widens to s 149–176 mm (u 189–216, ruling 1) at the same pose and
/// overlaps -102F2's interval (167–212).
#[test]
fn lq4_a_wider_part_widens_the_footprint_and_overlaps() {
    let mut s = session(MemoMode::Off);
    let q = s.query();
    let old: Id<Part> = q
        .table::<Placement>()
        .get(placement("-102F1"))
        .and_then(|p| p.part.as_ref().map(|r| r.copy))
        .expect("-102F1's placement names its part");
    let mut copy: LibCopy<Part> = LibCopy::clone(q.table::<LibCopy<Part>>().get(old).expect("the copy"));
    let item = ew_derive::ew_ids::Uuid::from_u128(0x4c51_0000_0000_4000_8000_0000_0000_0027);
    copy.item_id = Id::from_uuid(item);
    // A test library of its own: `part.assign` embeds copies of other libraries only.
    copy.library_id = Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(
        0x4c51_0000_0000_4000_8000_0000_0000_0001,
    ));
    copy.id = ew_derive::ew_ids::v5::lib_copy_id(copy.library_id.as_uuid(), item);
    copy.item.meta.key = Some("LQ4-27".parse().expect("a key"));
    copy.item.part_number = "LQ4-27".parse().expect("a part number");
    let mech = copy.item.mech.get_or_insert_with(Default::default);
    mech.width = Some(Nm(27_000_000));
    mech.height = Some(Nm(90_000_000));
    mech.rail_offset = Some(Pt2::new(Nm(9_000_000), Nm(42_500_000)));
    let doc = LibDocument::Part(copy.clone());
    let hash = ew_commands::commands::library_copy_hash(&doc).expect("a part hashes");
    copy.sha256 = Sha256(hash);
    let before = interval10(&q, carrier("R1"), placement("-102F1"));
    assert_eq!(before, (1490, 1670), "the 18 mm part about s = 158");
    run(
        &mut s,
        "part.assign",
        json!({"devices": [id_json(dev("-102F1"))], "part": id_json(copy.id),
               "copies": [serde_json::to_value(LibDocument::Part(copy.clone())).unwrap()],
               "mode": "replace", "from": id_json(old)}),
    );
    let q = s.query();
    let pose = q
        .table::<Placement>()
        .get(placement("-102F1"))
        .map(|p| p.pose)
        .unwrap();
    assert_eq!(pose.t, [Nm(158_000_000), Nm::ZERO, Nm::ZERO], "the same pose");
    let g = geometry(&q, placement("-102F1"));
    assert_eq!(g.part.map(|p| p.copy), Some(copy.id));
    assert_eq!(g.footprint, rect10((1890, 6975), (2160, 7875)));
    let f1 = interval10(&q, carrier("R1"), placement("-102F1"));
    let f2 = interval10(&q, carrier("R1"), placement("-102F2"));
    assert_eq!((f1, f2), ((1490, 1760), (1670, 2120)));
    assert!(f1.1 > f2.0, "the intervals overlap");
}

/// The label of -X1031 on the panel page 902 in the two modes, and T55 renumbered to T57 (`fields.set` of its frozen
/// tag) on a saved copy of the fixture: the label follows and no file under `layout/` changes (FR-LAY-006, D-02).
#[test]
fn lq5_labels_follow_the_tag_and_no_layout_file_changes() {
    let s = session(MemoMode::Off);
    let q = s.query();
    let label = |q: &Q, p: Id<Placement>, mode| q.get::<LayoutLabel>((p, page("902"), mode)).as_ref().clone();
    assert_eq!(label(&q, placement("-X1031"), LabelMode::Abbreviated), "-X1031");
    assert_eq!(
        label(&q, placement("-X1031"), LabelMode::Full),
        "=P01+J-E-2416-X1031"
    );

    let dir = ew_testdir::fresh_dir("ew-golden-layout-lq5");
    copy_dir(&golden_common::repo().join("golden/J-E-2416.ew"), &dir);
    let layout_files = || read_dir_bytes(&dir.join("layout"));
    let before = layout_files();
    assert_eq!(before.len(), 1, "one layout file");
    let mut options = StoreOptions::new("golden-layout-lq5");
    options.state_dir = Some(ew_testdir::fresh_dir("ew-golden-layout-lq5-state"));
    let (mut s, _) = Session::open(&dir, options, SessionOptions::default(), Recovery::Discard)
        .expect("the fixture copy opens");
    let t55 = placement("T55");
    let abbreviated = label(&s.query(), t55, LabelMode::Abbreviated);
    let full = label(&s.query(), t55, LabelMode::Full);
    assert_eq!(abbreviated, "T55");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "path": "tag.frozen", "targets": [id_json(dev("T55"))],
               "value": {"sign": "hidden", "text": "T57"}}),
    );
    assert_eq!(label(&s.query(), t55, LabelMode::Abbreviated), "T57");
    let renamed = label(&s.query(), t55, LabelMode::Full);
    assert!(renamed != full && renamed.ends_with("T57"), "{full} → {renamed}");
    s.save().expect("the copy saves");
    assert_eq!(layout_files(), before, "no file under layout/ changed");
    let _ = std::fs::remove_dir_all(&dir);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

fn read_dir_bytes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

/// Every value of the four memoised layout families at every key, memoised against cold, with the engine's oracle
/// on every read, at open and after each layout edit: a move, a flip, a place on the surface, a renumbering, an undo.
#[test]
fn the_layout_queries_memoise_as_they_run_cold() {
    let mut s = session(MemoMode::Oracle);
    let check = |s: &Session, state: &str| {
        let memo = s.query();
        let cold = s.query_over(&s.world().model);
        let spaces: Vec<_> = cold.table::<LayoutSpace>().ids().collect();
        let carriers: Vec<_> = cold.table::<Carrier>().ids().collect();
        let placements: Vec<_> = cold.table::<Placement>().ids().collect();
        for sp in &spaces {
            assert_eq!(
                memo.get::<Unplaced>(*sp),
                cold.get::<Unplaced>(*sp),
                "{state}: unplaced"
            );
        }
        for c in &carriers {
            assert_eq!(
                memo.get::<RailOccupancy>(*c),
                cold.get::<RailOccupancy>(*c),
                "{state}: occupancy"
            );
        }
        for p in &placements {
            assert_eq!(
                memo.get::<PlacementGeometry>(*p),
                cold.get::<PlacementGeometry>(*p),
                "{state}: geometry"
            );
            for mode in [LabelMode::Abbreviated, LabelMode::Full] {
                let key = (*p, page("902"), mode);
                assert_eq!(
                    memo.get::<LayoutLabel>(key),
                    cold.get::<LayoutLabel>(key),
                    "{state}: label"
                );
            }
        }
        assert!(
            s.take_oracle_mismatches().is_empty(),
            "{state}: the oracle found a mismatch"
        );
    };
    check(&s, "open");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-102G1")), "pose": {"t": [100_000_000, 0, 0], "q": quarter(0)}}),
    );
    check(&s, "move");
    run(
        &mut s,
        "layout.move",
        json!({"placement": id_json(placement("-201U2")), "pose": {"t": [79_000_000, 0, 0], "q": quarter(2)}}),
    );
    check(&s, "flip");
    run(
        &mut s,
        "layout.place",
        json!({"device": id_json(dev("F55")), "carrier": {"surface": id_json(surface())},
               "pose": {"t": [300_000_000, 50_000_000, 0], "q": quarter(1)}}),
    );
    check(&s, "place on the surface");
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "path": "tag.frozen", "targets": [id_json(dev("T55"))],
               "value": {"sign": "hidden", "text": "T57"}}),
    );
    check(&s, "renumber");
    s.undo(&ew_api::Origin::Gui).expect("an undo");
    check(&s, "undo");
}
