// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The EW-LAYOUT MVP rules (LC-1 to LC-7) on a small plate modelled on Golden A's (doc 20
//! §13, WP-S4.01 §6.1): a 650 × 900 mm plate in location +J-E-2416, rails R1 (40, 740)–(610, 740), R2 (40, 480)–
//! (610, 480) and R3 (40, 200)–(610, 200) (TS 35×7.5), ducts D2 (20, 860)–(20, 0) and D4 (40, 600)–(610, 600) (40 ×
//! 60). On R1 -101F1 (18 mm, s = 140), -102F1 (18 mm, s = 158) and -102F2 (45 mm, s = 189.5); on R2 the strip -X1031 (three
//! 6.2 mm terminals, left end s = 58.6); on R3 the station -201U1 (50 mm, s = 45) and its modules -201U2 (position 1,
//! s = 90), -201U3 (2, s = 130) and -201U4 (3, s = 170), 40 mm each. -F55 is mountable and unplaced. Golden A itself is
//! checked by `ew-api`'s AC-21 subset (`checks_part5.rs`), whose pack list holds EW-LAYOUT.
//!
//! The rules run at the project scope (the orchestrator's ruling 1: "No new scope kind. The EW-LAY rules run at the
//! project scope and enumerate the layout spaces, surfaces, carriers and placements through the layout queries").

use crate::common;

use common::*;
use ew_checks::{CheckMode, PRE_DELIVERY, Severity};
use ew_derive::layout::Quarter;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

const LOC: u128 = 0x10;
const LOC_206: u128 = 0x11;
const SPACE: u128 = 0x20;
const ENCLOSURE: u128 = 0x21;
const PLATE: u128 = 0x22;
const R1: u128 = 0x31;
const R2: u128 = 0x32;
const R3: u128 = 0x33;
const D2: u128 = 0x42;
const D4: u128 = 0x44;

const P_MCB: u128 = 0x600; // TS 35, 18 × 85 mm, centre clip
const P_MICO: u128 = 0x601; // TS 35, 45 × 90 mm
const P_WIDE: u128 = 0x602; // TS 35, 27 × 85 mm, clip (9, 42.5): LQ-4's test part
const P_SCREW: u128 = 0x603; // screw, 50 × 120 mm
const P_TERM: u128 = 0x604; // TS 35, 6.2 × 47 mm
const P_COUPLER: u128 = 0x605; // TS 35, 50 × 107 mm (195740)
const P_EXTRA: u128 = 0x606; // on parent (550208's role on -201U1)
const P_MOD: u128 = 0x607; // TS 35, 40 × 107 mm
const P_TS32: u128 = 0x608; // TS 32, 18 × 85 mm

const ENC_DEV: u128 = 0xe0;
const F101: u128 = 0xd1;
const F102: u128 = 0xd2;
const F102B: u128 = 0xd3;
const STRIP: u128 = 0xd4;
const U1: u128 = 0xd5;
const U2: u128 = 0xd6;
const U3: u128 = 0xd7;
const U4: u128 = 0xd8;
const F55: u128 = 0xd9;
const T1: u128 = 0xc1; // the strip's terminals T1, T1 + 1, …

const PL_F101: u128 = 0xf1;
const PL_F102: u128 = 0xf2;
const PL_F102B: u128 = 0xf3;
const PL_STRIP: u128 = 0xf4;
const PL_U1: u128 = 0xf5;
const PL_U2: u128 = 0xf6;
const PL_U3: u128 = 0xf7;
const PL_U4: u128 = 0xf8;

const RULES: [&str; 6] = [
    "EW-LAY-001",
    "EW-LAY-003",
    "EW-LAY-004",
    "EW-LAY-006",
    "EW-LAY-010",
    "EW-LAY-016",
];

fn mm(v: f64) -> i64 {
    (v * MM as f64).round() as i64
}

fn pose(t: [i64; 3], k: u8) -> Value {
    json!({"t": t, "q": Quarter::new(k).quat()})
}

fn rail_pose(s: f64) -> Value {
    pose([mm(s), 0, 0], 0)
}

fn on(carrier: u128) -> Value {
    json!({"carrier": us(carrier)})
}

fn rec(fx: &mut Fx, n: u128) -> &mut Value {
    let id = us(n);
    fx.records
        .iter_mut()
        .find(|r| r["id"] == json!(id))
        .expect("the record")
}

fn mech_part(fx: &mut Fx, n: u128, number: &str, mech: Value) {
    fx.part(n, number, &json!({"mech": mech}));
}

/// A drawn device in `loc` with the part uses `parts`, its tag frozen at `<letter><counter>` so that it prints as
/// Golden A's do (`+J-E-2416-F55`).
fn device(fx: &mut Fx, n: u128, letter: &str, counter: u32, loc: u128, parts: &[u128]) {
    fx.drawn_device(n, letter, Some(counter), P1);
    located(fx, n, letter, counter, loc);
    if !parts.is_empty() {
        fx.with_parts(n, parts);
    }
}

/// Sets the device's location and freezes its tag at `<letter><counter>`.
fn located(fx: &mut Fx, n: u128, letter: &str, counter: u32, loc: u128) {
    let r = rec(fx, n);
    r["structure"] = json!({"location": us(loc)});
    r["tag"]["frozen"] = json!({"text": format!("{letter}{counter}"), "sign": "hidden"});
}

fn place(fx: &mut Fx, n: u128, device: u128, part: Option<u128>, carrier: Value, pose: Value) {
    let mut r =
        json!({"$k": "placement", "id": us(n), "device": us(device), "carrier": carrier, "pose": pose});
    if let Some(p) = part {
        r["part"] = json!({"copy": us(p)});
    }
    fx.push(r);
}

fn carrier(fx: &mut Fx, n: u128, kind: &str, path: &[(f64, f64)], params: Value) {
    let path: Vec<Value> = path.iter().map(|(u, v)| json!([mm(*u), mm(*v)])).collect();
    fx.push(json!({"$k": "carrier", "id": us(n), "surface": us(PLATE), "kind": kind, "path": path, "params": params}));
}

fn ts35() -> Value {
    json!({"rail": {"profile": "ts35x7_5"}})
}

fn duct() -> Value {
    json!({"duct": {"width": mm(40.0), "height": mm(60.0)}})
}

/// The plate without its layout records (the devices and parts only).
fn devices_only() -> Fx {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "structure_node", "id": us(LOC), "aspect": "location", "code": "J-E-2416"}))
        .push(json!({"$k": "structure_node", "id": us(LOC_206), "aspect": "location", "code": "J-E-206"}));
    let ts = |w: f64, h: f64| json!({"width": mm(w), "height": mm(h), "mount_kind": "ts35x7_5"});
    mech_part(&mut fx, P_MCB, "A9F78116", ts(18.0, 85.0));
    mech_part(&mut fx, P_MICO, "MICO 4.10", ts(45.0, 90.0));
    mech_part(
        &mut fx,
        P_WIDE,
        "WIDE 27",
        json!({"width": mm(27.0), "height": mm(85.0), "mount_kind": "ts35x7_5", "rail_offset": [mm(9.0), mm(42.5)]}),
    );
    mech_part(
        &mut fx,
        P_SCREW,
        "SK 3102.000",
        json!({"width": mm(50.0), "height": mm(120.0), "mount_kind": "screw"}),
    );
    mech_part(&mut fx, P_TERM, "UK 5 N", ts(6.2, 47.0));
    mech_part(&mut fx, P_COUPLER, "195740", ts(50.0, 107.0));
    mech_part(&mut fx, P_EXTRA, "550208", json!({"mount_kind": "on_parent"}));
    mech_part(&mut fx, P_MOD, "533361", ts(40.0, 107.0));
    mech_part(
        &mut fx,
        P_TS32,
        "TS32 MCB",
        json!({"width": mm(18.0), "height": mm(85.0), "mount_kind": "ts32"}),
    );
    device(&mut fx, ENC_DEV, "UC", 1, LOC, &[]);
    device(&mut fx, F101, "F", 101, LOC, &[P_MCB]);
    device(&mut fx, F102, "F", 102, LOC, &[P_MCB]);
    device(&mut fx, F102B, "F", 103, LOC, &[P_MICO]);
    device(&mut fx, F55, "F", 55, LOC, &[P_MCB]);
    fx.device(STRIP, "X", Some(1031)).with("strip", json!({}));
    located(&mut fx, STRIP, "X", 1031, LOC);
    for i in 0..3 {
        terminal(&mut fx, T1 + i, i as u32 + 1);
    }
    device(&mut fx, U1, "U", 201, LOC, &[P_COUPLER, P_EXTRA]);
    for (n, counter, position) in [(U2, 202, 1), (U3, 203, 2), (U4, 204, 3)] {
        device(&mut fx, n, "U", counter, LOC, &[P_MOD]);
        rec(&mut fx, n)["parent"] = json!(us(U1));
        rec(&mut fx, n)["position"] = json!(position);
    }
    fx
}

/// A terminal of the strip with sequence `seq`, whose part is the 6.2 mm terminal.
fn terminal(fx: &mut Fx, n: u128, seq: u32) {
    fx.device(n, "X", None).with(
        "terminal",
        json!({"strip": us(STRIP), "seq": seq, "designation": seq.to_string()}),
    );
    fx.with("structure", json!({"location": us(LOC)}));
    fx.with_parts(n, &[P_TERM]);
}

/// The fixture: Golden A's plate in small, nothing wrong but -F55 unplaced.
fn plate() -> Fx {
    let mut fx = devices_only();
    let identity = pose([0, 0, 0], 0);
    let usable: Vec<Value> = [(0.0, 0.0), (650.0, 0.0), (650.0, 900.0), (0.0, 900.0)]
        .iter()
        .map(|(u, v)| json!([mm(*u), mm(*v)]))
        .collect();
    fx.push(json!({"$k": "layout_space", "id": us(SPACE), "location": us(LOC)}))
        .push(
            json!({"$k": "enclosure", "id": us(ENCLOSURE), "space": us(SPACE), "device": us(ENC_DEV),
                     "pose": identity}),
        )
        .push(
            json!({"$k": "mounting_surface", "id": us(PLATE), "enclosure": us(ENCLOSURE), "kind": "plate",
                     "frame": identity, "usable": usable}),
        );
    carrier(&mut fx, R1, "rail", &[(40.0, 740.0), (610.0, 740.0)], ts35());
    carrier(&mut fx, R2, "rail", &[(40.0, 480.0), (610.0, 480.0)], ts35());
    carrier(&mut fx, R3, "rail", &[(40.0, 200.0), (610.0, 200.0)], ts35());
    carrier(&mut fx, D2, "duct", &[(20.0, 860.0), (20.0, 0.0)], duct());
    carrier(&mut fx, D4, "duct", &[(40.0, 600.0), (610.0, 600.0)], duct());
    place(&mut fx, PL_F101, F101, Some(P_MCB), on(R1), rail_pose(140.0));
    place(&mut fx, PL_F102, F102, Some(P_MCB), on(R1), rail_pose(158.0));
    place(&mut fx, PL_F102B, F102B, Some(P_MICO), on(R1), rail_pose(189.5));
    place(&mut fx, PL_STRIP, STRIP, None, on(R2), rail_pose(58.6));
    place(&mut fx, PL_U1, U1, Some(P_COUPLER), on(R3), rail_pose(45.0));
    place(&mut fx, PL_U2, U2, Some(P_MOD), on(R3), rail_pose(90.0));
    place(&mut fx, PL_U3, U3, Some(P_MOD), on(R3), rail_pose(130.0));
    place(&mut fx, PL_U4, U4, Some(P_MOD), on(R3), rail_pose(170.0));
    fx
}

/// The findings of every EW-LAY rule but `except`, which must be none.
fn others_silent(m: &ew_model::Model, except: &[&str]) {
    for rule in RULES.iter().filter(|r| !except.contains(r)) {
        let f = findings_of(m, rule);
        assert!(f.is_empty(), "{rule}: {f:#?}");
    }
}

/// The messages of `rule`'s findings, sorted.
fn messages(m: &ew_model::Model, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = findings_of(m, rule).into_iter().map(|f| f.message).collect();
    out.sort();
    out
}

fn dutch(m: &ew_model::Model, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = findings_of(m, rule)
        .iter()
        .map(|f| ew_checks::messages::render(f, "nl-NL"))
        .collect();
    out.sort();
    out
}

const F55_UNPLACED: &str = "+J-E-2416-F55 is not placed in the layout of +J-E-2416; place it on a rail or on the \
                            mounting plate";

// ---------------------------------------------------------------------------------------------------------
// LC-1, LC-7: EW-LAY-001 and its release severity

#[test]
fn lc1_the_unplaced_device_is_reported_and_placing_it_clears_it() {
    let m = plate().build();
    let found = findings_of(&m, "EW-LAY-001");
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].message, F55_UNPLACED);
    assert_eq!(found[0].subject, "+J-E-2416-F55");
    assert_eq!(found[0].severity, Severity::Warning, "online: warning");
    assert_eq!(
        found[0].page, None,
        "an unplaced device is shown on no layout page"
    );
    assert_eq!(
        dutch(&m, "EW-LAY-001"),
        [
            "+J-E-2416-F55 is niet geplaatst in de layout van +J-E-2416; plaats het op een rail of op de montageplaat"
        ]
    );
    // the rest of the plate is clean: no part, location, overlap, carrier or order finding
    others_silent(&m, &["EW-LAY-001"]);

    let mut fx = plate();
    place(&mut fx, 0xf9, F55, Some(P_MCB), on(R1), rail_pose(400.0));
    let m = fx.build();
    others_silent(&m, &[]);
}

#[test]
fn lc1_a_project_without_a_layout_space_reports_none() {
    let m = devices_only().build();
    others_silent(&m, &[]);
}

/// LC-7 (the orchestrator's ruling 2: "LC-7's release wording is a finding filter ... the rule's `release_severity`
/// is error and the filter lives in its `run()`"): the pre-delivery release run reports EW-LAY-001 at error, and a
/// project without a layout space reports none.
#[test]
fn lc7_release_counts_the_unplaced_device_at_error_only_where_a_layout_space_exists() {
    let m = plate().build();
    let q = ew_derive::Q::new(&m);
    let report = ew_checks::run(&q, &PRE_DELIVERY, CheckMode::Release);
    let lay: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule.starts_with("EW-LAY-"))
        .collect();
    assert_eq!(lay.len(), 1, "{lay:#?}");
    assert_eq!(lay[0].rule, "EW-LAY-001");
    assert_eq!(lay[0].severity, Severity::Error);

    let m = devices_only().build();
    let q = ew_derive::Q::new(&m);
    let report = ew_checks::run(&q, &PRE_DELIVERY, CheckMode::Release);
    assert!(report.findings.iter().all(|f| !f.rule.starts_with("EW-LAY-")));
}

// ---------------------------------------------------------------------------------------------------------
// LC-2: EW-LAY-003

#[test]
fn lc2_removing_the_other_part_changes_nothing_removing_the_placed_one_is_reported() {
    let mut fx = plate();
    fx.with_parts(U1, &[P_COUPLER]);
    others_silent(&fx.build(), &["EW-LAY-001"]);

    let mut fx = plate();
    fx.with_parts(U1, &[P_EXTRA]);
    let m = fx.build();
    let found = findings_of(&m, "EW-LAY-003");
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        found[0].message,
        "the placement of +J-E-2416-U201 realises part 195740, which the device no longer uses; place the device \
         with one of its parts, or give it the part back"
    );
    assert!(
        found[0].page.is_none(),
        "no panel page shows the plate in this fixture"
    );
    // No quick fix when the device has no primary part (its one part mounts on its parent).
    assert!(found[0].fix.is_none());
    assert_eq!(
        dutch(&m, "EW-LAY-003"),
        [
            "de plaatsing van +J-E-2416-U201 gebruikt artikel 195740, dat het apparaat niet meer heeft; plaats het \
          apparaat met een van zijn artikelen, of geef het het artikel terug"
        ]
    );
}

/// WP-S4.02 D7 (AS-8): when the device has a primary part, EW-LAY-003 offers "Realise the primary part":
/// `layout.move` of the placement at its own pose with the primary part's copy (`ew-api`'s
/// `golden_layout_assembly.rs` runs it and the finding clears).
#[test]
fn lc2_the_quick_fix_realises_the_primary_part() {
    let mut fx = plate();
    fx.with_parts(U1, &[P_MOD]);
    let m = fx.build();
    let found = findings_of(&m, "EW-LAY-003");
    assert_eq!(found.len(), 1, "{found:#?}");
    let fix = found[0].fix.as_ref().expect("the quick fix");
    assert_eq!(fix.label, "Realise the primary part");
    assert_eq!(fix.command, "layout.move");
    assert_eq!(
        fix.params,
        json!({"placement": us(PL_U1), "pose": rail_pose(45.0), "part": us(P_MOD)})
    );
}

// ---------------------------------------------------------------------------------------------------------
// LC-3: EW-LAY-004

#[test]
fn lc3_a_device_of_another_location_placed_on_the_plate() {
    let mut fx = plate();
    device(&mut fx, 0xda, "S", 1, LOC_206, &[P_MCB]);
    place(&mut fx, 0xfa, 0xda, Some(P_MCB), on(R1), rail_pose(400.0));
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-004"),
        [
            "+J-E-206-S1 is in +J-E-206 but is placed in the layout of +J-E-2416; move it to the layout of its \
          location, or correct its location"
        ]
    );
    others_silent(&m, &["EW-LAY-001", "EW-LAY-004"]);
}

// ---------------------------------------------------------------------------------------------------------
// LC-4: EW-LAY-006

#[test]
fn lc4_the_wider_part_overlaps_its_neighbour() {
    // LQ-4's 27 mm part with its clip at (9, 42.5) at s = 158: 149 … 176 against -102F2's 167 … 212
    let mut fx = plate();
    fx.with_parts(F102, &[P_WIDE]);
    rec(&mut fx, PL_F102)["part"] = json!({"copy": us(P_WIDE)});
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-006"),
        ["+J-E-2416-F102 overlaps +J-E-2416-F103; move one of them"]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-006"),
        ["+J-E-2416-F102 overlapt +J-E-2416-F103; verplaats een van beide"]
    );
    others_silent(&m, &["EW-LAY-001", "EW-LAY-006"]);
}

#[test]
fn lc4_a_placement_over_a_duct_and_a_rail_across_a_duct() {
    let mut fx = plate();
    // the screw part's footprint (300, 560)–(350, 680) crosses D4's (40, 580)–(610, 620)
    device(&mut fx, 0xdb, "E", 1, LOC, &[P_SCREW]);
    place(
        &mut fx,
        0xfb,
        0xdb,
        Some(P_SCREW),
        json!({"surface": us(PLATE)}),
        pose([mm(300.0), mm(560.0), 0], 0),
    );
    // a rail from (0, 100) crosses D2's (0, 0)–(40, 860)
    carrier(&mut fx, 0x34, "rail", &[(0.0, 100.0), (300.0, 100.0)], ts35());
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-006"),
        [
            "+J-E-2416-E1 overlaps the duct at (40.0, 600.0)–(610.0, 600.0) mm; move the device or the duct",
            "the rail at (0.0, 100.0)–(300.0, 100.0) mm overlaps the duct at (20.0, 860.0)–(20.0, 0.0) mm; move the \
             rail or the duct",
        ]
    );
    others_silent(&m, &["EW-LAY-001", "EW-LAY-006"]);
}

#[test]
fn lc4_two_placements_of_one_device_overlap() {
    // cycle 41's ruling 2: one device, one placement per part, 18 mm and 6.2 mm at s = 400 and s = 405 on R1
    let mut fx = plate();
    device(&mut fx, 0xdc, "K", 1, LOC, &[P_MCB, P_TERM]);
    place(&mut fx, 0xfc, 0xdc, Some(P_MCB), on(R1), rail_pose(400.0));
    place(&mut fx, 0xfd, 0xdc, Some(P_TERM), on(R1), rail_pose(405.0));
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-006"),
        ["two placements of +J-E-2416-K1 overlap; move one of them"]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-006"),
        ["twee plaatsingen van +J-E-2416-K1 overlappen; verplaats een van beide"]
    );
}

/// The plate with `count` screw-mounted devices on the plate itself, the i-th at (620, `v(i)`) mm: right of every rail
/// and duct, so only their own footprints (50 × 120 mm) can overlap. The records are pushed whole (the fixture's
/// helpers search the records linearly).
fn screws(count: u128, v: impl Fn(u128) -> f64) -> ew_model::Model {
    let mut fx = plate();
    for i in 0..count {
        let (dev, pl) = (0x100_0000 + i, 0x200_0000 + i);
        fx.push(json!({
            "$k": "device", "id": us(dev),
            "tag": {"letter": "E", "counter": 1000 + i, "frozen": {"text": format!("E{}", 1000 + i), "sign": "hidden"}},
            "structure": {"location": us(LOC)},
            "parts": [{"part": {"copy": us(P_SCREW)}, "qty": "1"}],
        }));
        place(
            &mut fx,
            pl,
            dev,
            Some(P_SCREW),
            json!({"surface": us(PLATE)}),
            pose([mm(620.0), mm(v(i)), 0], 0),
        );
    }
    fx.build()
}

/// EW-LAY-006's findings, run alone (the other rules have nothing to say about the sweep).
fn collisions(m: &ew_model::Model) -> Vec<ew_checks::Finding> {
    use ew_checks::Rule;
    let found = ew_checks::rules::layout::Collision.run(&ew_derive::Q::new(m), &ew_checks::Scope::Project);
    assert_messages(&found);
    found
}

#[test]
fn lc4_five_thousand_footprints_at_one_spot_give_64_overlaps_and_one_summary() {
    // cycle 41's ruling 1: an output test, not a timing
    let m = screws(5000, |_| 0.0);
    let found = collisions(&m);
    assert_eq!(found.len(), 65, "64 pairs plus the summary");
    let summary: Vec<&ew_checks::Finding> = found
        .iter()
        .filter(|f| f.discriminator == "overlap_limit")
        .collect();
    assert_eq!(summary.len(), 1);
    assert_eq!(
        summary[0].message,
        "more footprints overlap on the mounting plate of +J-E-2416-UC1; the first 64 overlaps are listed"
    );
    assert_eq!(
        ew_checks::messages::render(summary[0], "nl-NL"),
        "meer voetafdrukken overlappen op de montageplaat van +J-E-2416-UC1; de eerste 64 overlappingen staan vermeld"
    );
    // the other 64 are pairs of two placements (their whole texts render from their arguments: `assert_messages`)
    let pairs = found
        .iter()
        .filter(|f| f.args.contains(&("form", "placements".to_owned())))
        .count();
    assert_eq!(pairs, 64);
}

#[test]
fn lc4_five_thousand_touching_footprints_in_one_column_give_none() {
    // each 120 mm tall, one above the other: they touch and do not overlap
    let m = screws(5000, |i| 120.0 * i as f64);
    let found = collisions(&m);
    assert!(found.is_empty(), "{found:#?}");
}

// ---------------------------------------------------------------------------------------------------------
// LC-5 and FT-845: EW-LAY-010

#[test]
fn lc5_profile_pose_footprint_strip_and_enclosure_limit() {
    let mut fx = plate();
    // a TS 32 rail with a TS 35 device on it
    carrier(
        &mut fx,
        0x35,
        "rail",
        &[(40.0, 100.0), (300.0, 100.0)],
        json!({"rail": {"profile": "ts32"}}),
    );
    device(&mut fx, 0xdc, "K", 1, LOC, &[P_MCB]);
    place(&mut fx, 0xfc, 0xdc, Some(P_MCB), on(0x35), rail_pose(100.0));
    // a pose beyond R1's end
    device(&mut fx, 0xdd, "K", 2, LOC, &[P_MCB]);
    place(&mut fx, 0xfd, 0xdd, Some(P_MCB), on(R1), rail_pose(600.0));
    // a footprint past the plate's right edge: (630, 10)–(680, 130)
    device(&mut fx, 0xde, "E", 2, LOC, &[P_SCREW]);
    place(
        &mut fx,
        0xfe,
        0xde,
        Some(P_SCREW),
        json!({"surface": us(PLATE)}),
        pose([mm(630.0), mm(10.0), 0], 0),
    );
    // -X1031 grown past R2's end: moved to s = 550, four terminals of 6.2 mm end at 574.8 > 570
    terminal(&mut fx, T1 + 3, 4);
    rec(&mut fx, PL_STRIP)["pose"] = rail_pose(550.0);
    // a second enclosure in the space
    device(&mut fx, 0xdf, "UC", 2, LOC, &[]);
    fx.push(
        json!({"$k": "enclosure", "id": us(0x29), "space": us(SPACE), "device": us(0xdf),
                   "pose": pose([0, 0, 0], 0)}),
    );
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "+J-E-2416-E2 lies outside the usable area of the mounting plate; move it inside",
            "+J-E-2416-K1 (part A9F78116) does not fit the rail at (40.0, 100.0)–(300.0, 100.0) mm (TS 32); place it \
             on a rail it fits",
            "+J-E-2416-K2 is placed at 600.0 mm on the rail at (40.0, 740.0)–(610.0, 740.0) mm, outside its 0.0 to \
             570.0 mm; move it onto the rail",
            "+J-E-2416-X1031 occupies 550.0 to 574.8 mm on the rail at (40.0, 480.0)–(610.0, 480.0) mm, beyond its \
             0.0 to 570.0 mm; shorten it or move it",
            "the layout of +J-E-2416 has a second enclosure, +J-E-2416-UC2; a layout space has one enclosure; remove \
             it",
        ]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-010")[3],
        "+J-E-2416-X1031 beslaat 550,0 tot 574,8 mm op de rail bij (40.0, 480.0)–(610.0, 480.0) mm, voorbij 0,0 tot \
         570,0 mm; maak het korter of verplaats het"
    );
}

#[test]
fn ft845_the_geometry_cases_the_layout_queries_leave_to_the_rule() {
    let mut fx = plate();
    // on a duct
    device(&mut fx, 0xdc, "K", 1, LOC, &[P_MCB]);
    place(&mut fx, 0xfc, 0xdc, Some(P_MCB), on(D4), rail_pose(100.0));
    // beside the rail's axis
    device(&mut fx, 0xdd, "K", 2, LOC, &[P_MCB]);
    place(
        &mut fx,
        0xfd,
        0xdd,
        Some(P_MCB),
        on(R2),
        pose([mm(400.0), mm(5.0), 0], 0),
    );
    // turned 90° on a rail
    device(&mut fx, 0xde, "K", 3, LOC, &[P_MCB]);
    place(
        &mut fx,
        0xfe,
        0xde,
        Some(P_MCB),
        on(R2),
        pose([mm(450.0), 0, 0], 1),
    );
    // off the plate's plane
    device(&mut fx, 0xdf, "E", 1, LOC, &[P_SCREW]);
    place(
        &mut fx,
        0xff,
        0xdf,
        Some(P_SCREW),
        json!({"surface": us(PLATE)}),
        pose([mm(300.0), mm(300.0), mm(1.0)], 0),
    );
    // a rail part on the plate itself
    device(&mut fx, 0xe1, "K", 4, LOC, &[P_MCB]);
    place(
        &mut fx,
        0xf101,
        0xe1,
        Some(P_MCB),
        json!({"surface": us(PLATE)}),
        pose([mm(400.0), mm(300.0), 0], 0),
    );
    // a diagonal rail and a duct past the plate's top edge
    carrier(&mut fx, 0x36, "rail", &[(100.0, 400.0), (200.0, 420.0)], ts35());
    carrier(&mut fx, 0x37, "duct", &[(100.0, 890.0), (300.0, 890.0)], duct());
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "+J-E-2416-E1 is placed off the plane of the mounting plate; place it on the plate",
            "+J-E-2416-K1 is placed on the duct at (40.0, 600.0)–(610.0, 600.0) mm, which carries no devices; place \
             it on a rail",
            "+J-E-2416-K2 is placed beside the axis of the rail at (40.0, 480.0)–(610.0, 480.0) mm; move it onto the \
             rail",
            "+J-E-2416-K3 is turned on the rail at (40.0, 480.0)–(610.0, 480.0) mm by an angle other than 0° or \
             180°; turn it back",
            "+J-E-2416-K4 (part A9F78116) does not mount on the plate itself; place it on a rail",
            "the duct at (100.0, 890.0)–(300.0, 890.0) mm lies outside the usable area of the mounting plate; move it \
             inside",
            "the path of the rail at (100.0, 400.0)–(200.0, 420.0) mm is not two points on one horizontal or \
             vertical line; edit its path",
        ]
    );
}

#[test]
fn d3_limits_a_second_space_and_a_second_plate() {
    let mut fx = plate();
    fx.push(json!({"$k": "layout_space", "id": us(0x2a), "location": us(LOC)}));
    fx.push(
        json!({"$k": "mounting_surface", "id": us(0x2b), "enclosure": us(ENCLOSURE), "kind": "plate",
                   "frame": pose([0, 0, 0], 0)}),
    );
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "+J-E-2416 has a second layout space; a location has one layout space; remove it",
            "the enclosure +J-E-2416-UC1 has a second mounting plate; an enclosure has one mounting plate; remove it",
        ]
    );
    let targets: Vec<_> = findings_of(&m, "EW-LAY-010")
        .iter()
        .map(|f| f.targets[0].kind)
        .collect();
    assert!(targets.contains(&ew_derive::ew_schema::Kind::LayoutSpace));
    assert!(targets.contains(&ew_derive::ew_schema::Kind::MountingSurface));
}

// ---------------------------------------------------------------------------------------------------------
// LC-6: EW-LAY-016

#[test]
fn lc6_swapping_two_modules_and_a_device_between_members() {
    let mut fx = plate();
    rec(&mut fx, PL_U3)["pose"] = rail_pose(170.0);
    rec(&mut fx, PL_U4)["pose"] = rail_pose(130.0);
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-016"),
        [
            "the members of +J-E-2416-U201 on the rail at (40.0, 200.0)–(610.0, 200.0) mm are out of order: \
          +J-E-2416-U203 belongs before +J-E-2416-U204; place the parent first, then the members by position"
        ]
    );
    others_silent(&m, &["EW-LAY-001", "EW-LAY-016"]);

    // -F55 placed between -201U3 and -201U4: -201U4 moves on to make room
    let mut fx = plate();
    rec(&mut fx, PL_U4)["pose"] = rail_pose(200.0);
    place(&mut fx, 0xf9, F55, Some(P_MCB), on(R3), rail_pose(159.0));
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-016"),
        [
            "+J-E-2416-F55 is placed between members of +J-E-2416-U201 on the rail at (40.0, 200.0)–(610.0, 200.0) mm; \
          keep the members together"
        ]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-016"),
        [
            "+J-E-2416-F55 staat tussen leden van +J-E-2416-U201 op de rail bij (40.0, 200.0)–(610.0, 200.0) mm; houd \
          de leden bij elkaar"
        ]
    );
    others_silent(&m, &["EW-LAY-016"]);
}

// ---------------------------------------------------------------------------------------------------------
// EB-4: EW-LAY-010's end-bracket cases

const P_EB: u128 = 0x609; // an end bracket, 10 mm wide
const STRIP2: u128 = 0xea; // -X1032, two 6.2 mm terminals (12.4 mm)
const T2: u128 = 0xb1; // its terminals
const PL_STRIP2: u128 = 0xf10a;
const K9: u128 = 0xeb;
const PL_K9: u128 = 0xf10b;

/// The plate with -X1031 and -X1032 each declaring the end bracket (D3), -X1031 moved to `s1031` and -X1032 placed on
/// R2 with its left end at `s`.
fn bracketed(s1031: f64, s: f64) -> Fx {
    let mut fx = plate();
    fx.part(
        P_EB,
        "EB 10",
        &json!({"kind": "end_bracket", "mech": {"width": mm(10.0)}}),
    );
    fx.with_parts(STRIP, &[P_EB]);
    fx.device(STRIP2, "X", Some(1032)).with("strip", json!({}));
    located(&mut fx, STRIP2, "X", 1032, LOC);
    fx.with_parts(STRIP2, &[P_EB]);
    for i in 0..2u32 {
        let n = T2 + u128::from(i);
        fx.device(n, "X", None).with(
            "terminal",
            json!({"strip": us(STRIP2), "seq": i + 1, "designation": (i + 1).to_string()}),
        );
        fx.with("structure", json!({"location": us(LOC)}));
        fx.with_parts(n, &[P_TERM]);
    }
    rec(&mut fx, PL_STRIP)["pose"] = rail_pose(s1031);
    place(&mut fx, PL_STRIP2, STRIP2, None, on(R2), rail_pose(s));
    fx
}

#[test]
fn eb4_a_bracket_overlapping_a_strip_in_its_run_and_a_placement() {
    // -X1031 [58.6, 77.2], -X1032 5 mm after it [82.2, 94.6]: one run, the bracket after -X1031 [77.2, 87.2] overlaps
    // -X1032. K9 [40, 58] under the run-start bracket [48.6, 58.6].
    let mut fx = bracketed(58.6, 82.2);
    device(&mut fx, K9, "K", 9, LOC, &[P_MCB]);
    place(&mut fx, PL_K9, K9, Some(P_MCB), on(R2), rail_pose(49.0));
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "The end bracket after +J-E-2416-X1031 overlaps +J-E-2416-X1032 on rail 2.",
            "The end bracket before +J-E-2416-X1031 overlaps +J-E-2416-K9 on rail 2.",
        ]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-010"),
        [
            "De eindsteun na +J-E-2416-X1031 overlapt +J-E-2416-X1032 op rail 2.",
            "De eindsteun vóór +J-E-2416-X1031 overlapt +J-E-2416-K9 op rail 2.",
        ]
    );
    // Each targets the strip and its placement; the bracket's side and start tell the two apart (ruling 12).
    let mut found: Vec<(String, Vec<String>)> = findings_of(&m, "EW-LAY-010")
        .into_iter()
        .map(|f| {
            (
                f.discriminator,
                f.targets.iter().map(|t| t.id.as_uuid().to_string()).collect(),
            )
        })
        .collect();
    found.sort();
    let mut targets = vec![us(STRIP), us(PL_STRIP)];
    targets.sort();
    assert_eq!(
        found,
        [
            ("bracket_overlap:after:77200000".to_owned(), targets.clone()),
            ("bracket_overlap:before:48600000".to_owned(), targets),
        ]
    );
}

#[test]
fn eb4_a_bracket_overlapping_another_bracket_and_brackets_past_the_rail_ends() {
    // K9 [88.2, 106.2] between the strips makes two runs: the bracket after -X1031 [77.2, 87.2] overlaps the one before
    // -X1032 [82, 92], which overlaps K9, as does the one after -X1032 [104.4, 114.4].
    let mut fx = bracketed(58.6, 92.0);
    device(&mut fx, K9, "K", 9, LOC, &[P_MCB]);
    place(&mut fx, PL_K9, K9, Some(P_MCB), on(R2), rail_pose(97.2));
    let m = fx.build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "The end bracket after +J-E-2416-X1031 overlaps the end bracket before +J-E-2416-X1032 on rail 2.",
            "The end bracket after +J-E-2416-X1032 overlaps +J-E-2416-K9 on rail 2.",
            "The end bracket before +J-E-2416-X1032 overlaps +J-E-2416-K9 on rail 2.",
        ]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-010")[0],
        "De eindsteun na +J-E-2416-X1031 overlapt de eindsteun vóór +J-E-2416-X1032 op rail 2."
    );

    // -X1031 at s = 4 mm and -X1032 ending at R2's end (570 mm, no overflow of its own): the bracket before -X1031
    // [-6, 4] and the one after -X1032 [570, 580] leave the rail.
    let m = bracketed(4.0, 557.6).build();
    assert_eq!(
        messages(&m, "EW-LAY-010"),
        [
            "The end bracket after +J-E-2416-X1032 passes the end of rail 2 by 10 mm.",
            "The end bracket before +J-E-2416-X1031 passes the start of rail 2 by 6 mm.",
        ]
    );
    assert_eq!(
        dutch(&m, "EW-LAY-010"),
        [
            "De eindsteun na +J-E-2416-X1032 steekt 10 mm voorbij het einde van rail 2.",
            "De eindsteun vóór +J-E-2416-X1031 steekt 6 mm voorbij het begin van rail 2.",
        ]
    );
}

#[test]
fn eb4_brackets_with_room_give_no_finding() {
    // -X1032 10 mm after -X1031: one bracket between, nothing overlaps, nothing leaves the rail.
    let m = bracketed(58.6, 87.2).build();
    assert_eq!(messages(&m, "EW-LAY-010"), Vec::<String>::new());
}

const P_EB0: u128 = 0x60a; // an end bracket without a width
const P_TERM0: u128 = 0x60b; // a terminal without a width (TS 35, 0 × 47 mm)
const STRIP0: u128 = 0xec; // -X1040, one terminal of no width
const PL_STRIP0: u128 = 0xf10c;

/// The plate with -X1040, a strip of no width declaring a bracket of no width, placed on R2 at `s`: its run-start
/// bracket and the one after it are both `[s, s]`.
fn zero_width_strip(s: f64) -> Fx {
    strip_with_bracket_of_no_width(s, P_TERM0)
}

/// The plate with -X1040, a strip of one terminal of the part `terminal` declaring a bracket of no width, placed on R2
/// at `s`.
fn strip_with_bracket_of_no_width(s: f64, terminal: u128) -> Fx {
    let mut fx = plate();
    fx.part(P_EB0, "EB 0", &json!({"kind": "end_bracket"}));
    mech_part(
        &mut fx,
        P_TERM0,
        "UK 0",
        json!({"width": 0, "height": mm(47.0), "mount_kind": "ts35x7_5"}),
    );
    fx.device(STRIP0, "X", Some(1040)).with("strip", json!({}));
    located(&mut fx, STRIP0, "X", 1040, LOC);
    fx.with_parts(STRIP0, &[P_EB0]);
    fx.device(0xed, "X", None).with(
        "terminal",
        json!({"strip": us(STRIP0), "seq": 1, "designation": "1"}),
    );
    fx.with("structure", json!({"location": us(LOC)}));
    fx.with_parts(0xed, &[terminal]);
    place(&mut fx, PL_STRIP0, STRIP0, None, on(R2), rail_pose(s));
    fx
}

/// A strip with a width whose bracket part has none, placed 20 mm before R2's start: its run-start bracket
/// `[-20, -20]` reads "before" though it has no length and no other bracket shares its point; the one after it
/// `[-13.8, -13.8]` reads "after".
#[test]
fn eb4_the_run_start_bracket_of_no_width_reads_before_off_the_rail_start() {
    let m = strip_with_bracket_of_no_width(-20.0, P_TERM).build();
    let mut found: Vec<(String, String)> = findings_of(&m, "EW-LAY-010")
        .into_iter()
        .map(|f| (f.discriminator, f.message))
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            (
                "bracket_outside:after:-13800000".to_owned(),
                "The end bracket after +J-E-2416-X1040 passes the start of rail 2 by 13.8 mm.".to_owned()
            ),
            (
                "bracket_outside:before:-20000000".to_owned(),
                "The end bracket before +J-E-2416-X1040 passes the start of rail 2 by 20 mm.".to_owned()
            ),
            (
                "off_rail".to_owned(),
                "+J-E-2416-X1040 is placed at -20.0 mm on the rail at (40.0, 480.0)–(610.0, 480.0) mm, outside its \
                 0.0 to 570.0 mm; move it onto the rail"
                    .to_owned()
            ),
        ]
    );
}

/// Ruling 12: a strip whose bracket part has no width gives two brackets at one start; their findings never
/// share a fingerprint, and a zero-length bracket overlaps nothing.
#[test]
fn eb4_brackets_of_no_width_never_share_a_fingerprint_and_overlap_nothing() {
    // Past R2's end (570 mm): both brackets [600, 600] leave the rail, the run-start one "before" its strip.
    let m = zero_width_strip(600.0).build();
    let mut found: Vec<(String, String)> = findings_of(&m, "EW-LAY-010")
        .into_iter()
        .map(|f| (f.discriminator, f.message))
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            (
                "bracket_outside:after:600000000".to_owned(),
                "The end bracket after +J-E-2416-X1040 passes the end of rail 2 by 30 mm.".to_owned()
            ),
            (
                "bracket_outside:before:600000000".to_owned(),
                "The end bracket before +J-E-2416-X1040 passes the end of rail 2 by 30 mm.".to_owned()
            ),
            (
                "off_rail".to_owned(),
                "+J-E-2416-X1040 is placed at 600.0 mm on the rail at (40.0, 480.0)–(610.0, 480.0) mm, outside its \
                 0.0 to 570.0 mm; move it onto the rail"
                    .to_owned()
            ),
        ]
    );
    // Inside -X1031's footprint [58.6, 77.2]: the two brackets at 67 mm have no length and overlap nothing.
    let m = zero_width_strip(67.0).build();
    assert_eq!(messages(&m, "EW-LAY-010"), Vec::<String>::new());
}

/// Ruling 11: 200 strips stacked on one rail at s = 100 mm, each one 6.2 mm terminal declaring the 10 mm
/// bracket. One run: its run-start bracket [90, 100] (the first strip's) and one bracket after each strip, all at
/// [106.2, 116.2], each overlapping the next. EW-LAY-010 gives one finding per overlapping bracket (199, no cap), and
/// each strip's bracket line counts its own brackets (the first 2, every other 1; the BOM row 201). The speed is not
/// measured (owner's rule).
#[test]
fn eb4_a_rail_of_stacked_strips_gives_one_finding_per_bracket_and_its_bom_line() {
    const N: u32 = 200;
    let strip = |i: u32| 0x1000 + u128::from(i);
    let mut fx = plate();
    fx.part(
        P_EB,
        "EB 10",
        &json!({"kind": "end_bracket", "mech": {"width": mm(10.0)}}),
    );
    for i in 0..N {
        fx.device(strip(i), "X", Some(2000 + i)).with("strip", json!({}));
        located(&mut fx, strip(i), "X", 2000 + i, LOC);
        fx.with_parts(strip(i), &[P_EB]);
        let t = 0x2000 + u128::from(i);
        fx.device(t, "X", None).with(
            "terminal",
            json!({"strip": us(strip(i)), "seq": 1, "designation": "1"}),
        );
        fx.with("structure", json!({"location": us(LOC)}));
        fx.with_parts(t, &[P_TERM]);
        place(
            &mut fx,
            0x3000 + u128::from(i),
            strip(i),
            None,
            on(R2),
            rail_pose(100.0),
        );
    }
    let m = fx.build();
    let mut expected: Vec<String> = (0..N - 1)
        .map(|i| {
            format!(
                "The end bracket after +J-E-2416-X{} overlaps the end bracket after +J-E-2416-X{} on rail 2.",
                2000 + i,
                2001 + i
            )
        })
        .collect();
    expected.sort();
    assert_eq!(messages(&m, "EW-LAY-010"), expected);

    let q = ew_derive::Q::new(&m);
    for i in 0..N {
        let lines: Vec<(String, ew_derive::LineSource)> = q
            .get::<ew_derive::DevicePartLines>(ew_derive::ew_ids::Id::from_uuid(
                ew_derive::ew_ids::Uuid::from_u128(strip(i)),
            ))
            .iter()
            .filter(|l| l.source == ew_derive::LineSource::GroupAccessory)
            .map(|l| (l.qty.to_string(), l.source))
            .collect();
        let qty = if i == 0 { "2" } else { "1" };
        assert_eq!(
            lines,
            [(qty.to_owned(), ew_derive::LineSource::GroupAccessory)],
            "X{}",
            2000 + i
        );
    }
    let rows: Vec<(String, String)> = q
        .get::<ew_derive::BomRows>(ew_derive::BomMode::Component)
        .iter()
        .filter(|r| r.part_number == "EB 10")
        .map(|r| (r.part_number.clone(), r.qty.to_string()))
        .collect();
    assert_eq!(rows, [("EB 10".to_owned(), "201".to_owned())]);
}
