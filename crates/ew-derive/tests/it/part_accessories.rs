// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the parts bring with them on a small fixture (doc 07 §2.5, doc 12 §3.4): a strip's end plate after the last
//! terminal whose part names it, a choice that takes it off or swaps it, the end brackets of a rail group (k abutting
//! strips take k + 1, a strip on no rail two), a strip's older end-bracket part use that keeps its count, the width
//! the strip takes on its rail, and the parts list's accessory lines.

use crate::common;

use common::{Fx, LIBRARY, uid};
use ew_derive::layout::{GroupAccessories, StripItemKind, StripLayout};
use ew_derive::{DevicePartLines, LineSource, PartAccessories, Q, strip_bracket};
use ew_ids::{Id, Uuid};
use ew_schema::{AccessoryPer, Part};
use ew_units::Dec3;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

const T_A: u128 = 0x100; // terminal part naming the end plate per strip and the bracket per rail group
const T_B: u128 = 0x101; // terminal part naming the end plate per strip only
const EP: u128 = 0x102; // end plate, 2 mm
const EP2: u128 = 0x103; // another end plate, 3 mm
const EB: u128 = 0x104; // end bracket, 10 mm
const EB_OLD: u128 = 0x105; // an end bracket a strip holds as a part use, 8 mm

const LOC: u128 = 0x10;
const SPACE: u128 = 0x20;
const ENCLOSURE: u128 = 0x21;
const SURFACE: u128 = 0x22;
const RAIL: u128 = 0x23;

/// The copy ID of the starter-like library item `n`: `v5(NS_LIBCOPY, LIBRARY ‖ item n)`, so that a declaration's
/// library reference resolves to it.
fn copy(n: u128) -> Id<Part> {
    ew_ids::v5::lib_copy_id(Uuid::from_u128(LIBRARY), Uuid::from_u128(n))
}

fn libref(n: u128) -> Value {
    json!({"library": uid(LIBRARY), "item": uid(n), "version_req": "^1"})
}

fn part(fx: &mut Fx, n: u128, kind: &str, width_mm: i64, accessories: Value) {
    let mut doc = json!({
        "$k": "part", "id": copy(n).to_string(), "library_id": uid(LIBRARY), "item_id": uid(n),
        "version": "1.0.0", "sha256": common::sha((n % 251) as u8),
        "kind": kind, "part_number": format!("P{n:x}"),
        "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
        "mech": {"width": width_mm * MM, "height": 45 * MM, "mount_kind": "ts35x7_5"},
    });
    if accessories != json!([]) {
        doc["accessories"] = accessories;
    }
    fx.lib(doc);
}

fn strip(fx: &mut Fx, n: u128, counter: u32, terminals: &[(u128, u128)], extra: Value) {
    fx.device(n, "X", Some(counter)).with("strip", json!({}));
    for (k, v) in extra.as_object().expect("an object") {
        fx.with(k, v.clone());
    }
    for (seq, (t, p)) in terminals.iter().enumerate() {
        fx.device(*t, "X", None)
            .with(
                "terminal",
                json!({"strip": uid(n), "seq": seq + 1, "designation": (seq + 1).to_string()}),
            )
            .with(
                "parts",
                json!([{"part": {"copy": copy(*p).to_string()}, "qty": "1"}]),
            );
    }
}

fn place(fx: &mut Fx, n: u128, device: u128, s_mm: i64) {
    fx.push(
        json!({"$k": "placement", "id": uid(n), "device": uid(device), "carrier": {"carrier": uid(RAIL)},
                   "pose": {"t": [s_mm * MM, 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
    );
}

/// Two terminal parts, the end plates and brackets, one rail of 500 mm on a plate.
fn base() -> Fx {
    let mut fx = Fx::new();
    let per =
        |p: u128, per: &str, kind: &str| json!({"part": libref(p), "qty": "1", "per": per, "kind": kind});
    part(
        &mut fx,
        T_A,
        "terminal",
        5,
        json!([per(EP, "strip", "end_plate"), per(EB, "rail_group", "end_clamp")]),
    );
    part(
        &mut fx,
        T_B,
        "terminal",
        5,
        json!([per(EP, "strip", "end_plate")]),
    );
    part(&mut fx, EP, "accessory", 2, json!([]));
    part(&mut fx, EP2, "accessory", 3, json!([]));
    part(&mut fx, EB, "end_bracket", 10, json!([]));
    part(&mut fx, EB_OLD, "end_bracket", 8, json!([]));
    let identity = json!({"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]});
    fx.node(LOC, "location", "A", None)
        .push(json!({"$k": "layout_space", "id": uid(SPACE), "location": uid(LOC)}))
        .push(json!({"$k": "enclosure", "id": uid(ENCLOSURE), "space": uid(SPACE), "device": uid(0xe0),
                     "pose": identity}))
        .push(json!({"$k": "mounting_surface", "id": uid(SURFACE), "enclosure": uid(ENCLOSURE), "kind": "plate",
                     "frame": identity}))
        .push(json!({"$k": "carrier", "id": uid(RAIL), "surface": uid(SURFACE), "kind": "rail",
                     "path": [[0, 300 * MM], [500 * MM, 300 * MM]], "params": {"rail": {"profile": "ts35x7_5"}}}));
    fx.device(0xe0, "U", Some(1));
    fx
}

fn accessories(q: &Q, device: u128) -> Vec<(Id<Part>, Dec3, AccessoryPer, Option<u128>)> {
    q.get::<PartAccessories>(common::id(device))
        .iter()
        .map(|a| {
            (
                a.part.copy,
                a.qty,
                a.per.clone(),
                a.after.map(|t| t.as_uuid().as_u128()),
            )
        })
        .collect()
}

/// A strip of a TA and a TB terminal takes one end plate, after the TB terminal, the last that names it (both name
/// it), and two end brackets on no rail; a choice without a part takes the end plate off, one with another part swaps
/// it.
#[test]
fn a_strip_takes_its_end_plate_once_after_the_last_terminal_that_names_it() {
    let mut fx = base();
    strip(&mut fx, 0xa0, 1, &[(0xa1, T_A), (0xa2, T_B)], json!({}));
    strip(
        &mut fx,
        0xb0,
        2,
        &[(0xb1, T_A)],
        json!({"accessory_choices": [{"of": copy(EP).to_string()}]}),
    );
    strip(
        &mut fx,
        0xc0,
        3,
        &[(0xc1, T_A)],
        json!({"accessory_choices": [{"of": copy(EP).to_string(), "part": {"copy": copy(EP2).to_string()}}]}),
    );
    let model = fx.build();
    let q = Q::new(&model);
    let two = Dec3::from_int(2).unwrap();
    assert_eq!(
        accessories(&q, 0xa0),
        [
            (copy(EP), Dec3::ONE, AccessoryPer::Strip, Some(0xa2)),
            (copy(EB), two, AccessoryPer::RailGroup, None),
        ]
    );
    let with: Vec<_> = q.get::<PartAccessories>(common::id(0xa0))[0].with.clone();
    assert_eq!(with, [copy(T_A), copy(T_B)]);
    assert_eq!(
        accessories(&q, 0xb0),
        [(copy(EB), two, AccessoryPer::RailGroup, None)]
    );
    assert_eq!(accessories(&q, 0xc0)[0].0, copy(EP2));
    // The strip's items in mounting order: its terminals, the end plate after the second.
    let items: Vec<(bool, Option<i128>, i128)> = q
        .get::<StripLayout>(common::id(0xa0))
        .iter()
        .map(|i| (matches!(i.kind, StripItemKind::Accessory(_)), i.width, i.s))
        .collect();
    let mm = i128::from(MM);
    assert_eq!(
        items,
        [
            (false, Some(5 * mm), 0),
            (false, Some(5 * mm), 5 * mm),
            (true, Some(2 * mm), 10 * mm)
        ]
    );
    // The parts list counts them, each on its own line.
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(common::id(0xa0))
        .iter()
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(
        lines,
        [
            (copy(EP), Dec3::ONE, LineSource::Accessory { rail_group: false }),
            (copy(EB), two, LineSource::Accessory { rail_group: true }),
        ]
    );
}

/// Two abutting strips on a rail take three end brackets (k + 1), each strip's footprint holds its end plate, and
/// a strip that holds an end bracket as a part use keeps its count on that line and asks its terminals for none.
#[test]
fn abutting_strips_take_k_plus_one_brackets_and_an_older_part_use_keeps_its_count() {
    let mut fx = base();
    strip(&mut fx, 0xa0, 1, &[(0xa1, T_A), (0xa2, T_A)], json!({}));
    strip(&mut fx, 0xb0, 2, &[(0xb1, T_A)], json!({}));
    // -X1 is 5 + 5 + 2 = 12 mm wide from 20 mm; -X2 follows after one 10 mm bracket.
    place(&mut fx, 0x51, 0xa0, 20);
    place(&mut fx, 0x52, 0xb0, 42);
    strip(
        &mut fx,
        0xc0,
        3,
        &[(0xc1, T_A)],
        json!({"parts": [{"part": {"copy": copy(EB_OLD).to_string()}, "qty": "4"}]}),
    );
    let model = fx.build();
    let q = Q::new(&model);
    let brackets = q.get::<GroupAccessories>(common::id(RAIL));
    assert_eq!(brackets.len(), 3);
    assert_eq!(brackets.count_of(common::id(0xa0)), 2);
    assert_eq!(brackets.count_of(common::id(0xb0)), 1);
    let rail_group = |d: u128| {
        accessories(&q, d)
            .into_iter()
            .filter(|a| a.2 == AccessoryPer::RailGroup)
            .map(|a| a.1)
            .collect::<Vec<_>>()
    };
    assert_eq!(rail_group(0xa0), [Dec3::from_int(2).unwrap()]);
    assert_eq!(rail_group(0xb0), [Dec3::ONE]);
    // The older part use: its own line, its authored quantity on no rail, no declared bracket.
    let old = q.devices().get(common::id(0xc0)).cloned().unwrap();
    assert_eq!(strip_bracket(&q, &old).map(|b| b.part.copy), Some(copy(EB_OLD)));
    assert!(rail_group(0xc0).is_empty());
    let lines: Vec<(Id<Part>, Dec3, LineSource)> = q
        .get::<DevicePartLines>(common::id(0xc0))
        .iter()
        .map(|l| (l.part, l.qty, l.source))
        .collect();
    assert_eq!(
        lines[0],
        (copy(EB_OLD), Dec3::from_int(4).unwrap(), LineSource::Device)
    );
}

/// A part use of a variant is a parts-list row of its own that prints the variant's number and description (doc 07
/// §2.5 "Variants"); the part's own use prints the part's.
#[test]
fn a_variant_prints_its_own_number_on_its_own_row() {
    let mut fx = base();
    let doc = fx
        .lib
        .iter_mut()
        .find(|d| d["id"] == json!(copy(T_A).to_string()))
        .expect("the terminal part");
    doc["variants"] = json!([{"key": "BU", "name": {"en": "Blue"}, "part_number": "P100-BU",
                              "description": {"en": "Blue terminal"}}]);
    strip(&mut fx, 0xa0, 1, &[(0xa1, T_A), (0xa2, T_A)], json!({}));
    let r = fx
        .records
        .iter_mut()
        .find(|r| r["id"] == json!(uid(0xa2)))
        .expect("the second terminal");
    r["parts"][0]["part"]["variant"] = json!("BU");
    let model = fx.build();
    let q = Q::new(&model);
    let rows = q.get::<ew_derive::BomRows>(ew_derive::BomMode::Component);
    let mut printed: Vec<(String, Option<String>, String)> = rows
        .iter()
        .filter(|r| r.key.part == copy(T_A))
        .map(|r| {
            (
                r.part_number.clone(),
                r.key.variant.as_ref().map(|v| v.as_str().to_owned()),
                r.description.neutral_or_first().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    printed.sort();
    assert_eq!(
        printed,
        [
            ("P100".to_owned(), None, String::new()),
            (
                "P100-BU".to_owned(),
                Some("BU".to_owned()),
                "Blue terminal".to_owned()
            ),
        ]
    );
}
