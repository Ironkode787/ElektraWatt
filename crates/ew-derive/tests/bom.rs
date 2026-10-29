// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bill of materials (doc 20 §14.1, §14.2): part lines, aggregation, row order and assembly mode.
//!
//! Devices are placed at one common point, so rows of one page are told apart by part number only (the
//! anchor-cell tie-break is covered where cells exist).

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::{BomMode, BomRow, BomRows, DevicePartLines, LineSource, PageRef, Q};
use ew_ids::Id;
use ew_schema::OrderUnit;
use ew_units::Dec3;
use serde_json::{Value, json};

const PAGE3: u128 = 0x3;
const PAGE101: u128 = 0x101;
const PAGE103: u128 = 0x103;
const PAGE201: u128 = 0x201;
const PAGE202: u128 = 0x202;
const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;

// Parts (copy IDs).
const P_PROFIBUS_PLUG: u128 = 0x601; // 532216
const P_MCB_A: u128 = 0x602; // A9P52610
const P_MCB_B: u128 = 0x603; // 5SY4204-7
const P_UK5N: u128 = 0x604; // UK 5 N
const P_USLKG5: u128 = 0x605; // USLKG 5
const P_PT25: u128 = 0x606; // PT 2.5-TWIN
const P_END_BRACKET: u128 = 0x607;
const P_LINK_BLOCK: u128 = 0x608; // 550208
const P_ENCLOSURE: u128 = 0x609;
const P_PLATE: u128 = 0x60a;
const P_RAIL: u128 = 0x60b;
const P_DUCT: u128 = 0x60c;
const P_SPARE: u128 = 0x60d;
const P_BUS_NODE: u128 = 0x60e; // 195740
const P_CONF_CPX: u128 = 0x60f;
const P_CONF_MPA: u128 = 0x610;
const P_MODULE: u128 = 0x611; // 533361
const P_VALVE: u128 = 0x612; // 571334

fn part(fx: &mut Fx, n: u128, number: &str, kind: &str, unit: &str, description: &str) {
    fx.lib_item(
        "part",
        n,
        json!({
            "meta": {},
            "kind": kind,
            "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
            "part_number": number,
            "description": description,
            "unit": unit,
        }),
    );
}

/// Pages, one function definition with a symbol, and every part.
fn base() -> Fx {
    let mut fx = Fx::new();
    for (n, folio) in [
        (PAGE3, 3),
        (PAGE101, 101),
        (PAGE103, 103),
        (PAGE201, 201),
        (PAGE202, 202),
    ] {
        fx.page(n, folio, None, "multi_line", &[], "Page");
    }
    fx.fndef(
        FNDEF,
        ["ProtectionOvercurrent", "MCB", "1P"],
        &[(0, "1"), (1, "2")],
    );
    fx.symbol(
        SYM,
        FNDEF,
        json!("origin"),
        &[(0, 0, 0, 0), (1, 1, 0, 16_000_000)],
    );
    let parts = [
        (P_PROFIBUS_PLUG, "532216", "device", "piece", "PROFIBUS DP plug"),
        (
            P_MCB_A,
            "A9P52610",
            "device",
            "piece",
            "Miniature circuit breaker 1P+N, C, 10 A",
        ),
        (
            P_MCB_B,
            "5SY4204-7",
            "device",
            "piece",
            "Miniature circuit breaker 1P+N, C, 4 A",
        ),
        (P_UK5N, "UK 5 N", "terminal", "piece", "Feed-through terminal"),
        (P_USLKG5, "USLKG 5", "terminal", "piece", "PE terminal"),
        (
            P_PT25,
            "PT 2.5-TWIN",
            "terminal",
            "piece",
            "Feed-through terminal, 3 connections",
        ),
        (P_END_BRACKET, "C-EB", "end_bracket", "piece", "End bracket"),
        (
            P_LINK_BLOCK,
            "550208",
            "device",
            "piece",
            "Interlinking block with supply",
        ),
        (
            P_ENCLOSURE,
            "NSYPLA1074G",
            "enclosure",
            "piece",
            "Enclosure, polyester",
        ),
        (P_PLATE, "NSYPMM107", "mounting_plate", "piece", "Mounting plate"),
        (P_RAIL, "A-RAIL", "rail", "m", "DIN rail TS 35"),
        (P_DUCT, "B-DUCT", "duct", "m", "Wiring duct 40x60"),
        (P_SPARE, "D-SPARE", "device", "piece", "Spare"),
        (P_BUS_NODE, "195740", "device", "piece", "CPX-FB13 bus node"),
        (
            P_CONF_CPX,
            "51E-F13GCQP-D+H2GG",
            "configured",
            "set",
            "CPX terminal",
        ),
        (P_CONF_MPA, "32P-VGL-R", "configured", "set", "MPA valve terminal"),
        (P_MODULE, "533361", "device", "piece", "Electronic module"),
        (
            P_VALVE,
            "571334",
            "device",
            "piece",
            "5/2 monostable solenoid valve",
        ),
    ];
    for (n, number, kind, unit, d) in parts {
        part(&mut fx, n, number, kind, unit, d);
    }
    fx
}

fn part_uses(parts: &[(u128, &str)]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|(p, qty)| json!({"part": {"copy": uid(*p)}, "qty": qty}))
            .collect(),
    )
}

/// Places device `n` (function `n + 1`, representation `n + 2`) on `page`, all at one point.
fn place(fx: &mut Fx, n: u128, page: u128) {
    fx.function(n + 1, Some(n), FNDEF, true)
        .rep(n + 2, page, SYM, &[n + 1], 40_000_000, 40_000_000);
}

/// A device with a frozen tag and part uses, placed on `page` when given.
fn dev(fx: &mut Fx, n: u128, tag: &str, hidden: bool, parts: &[(u128, &str)], page: Option<u128>) {
    let mut frozen = json!({"text": tag});
    if hidden {
        frozen["sign"] = json!("hidden");
    }
    fx.push(json!({
        "$k": "device", "id": uid(n), "tag": {"letter": "F", "frozen": frozen},
        "parts": part_uses(parts),
    }));
    if let Some(page) = page {
        place(fx, n, page);
    }
}

/// A strip device with a frozen tag.
fn strip(fx: &mut Fx, n: u128, tag: &str, hidden: bool, accessories: &[u128]) {
    let mut frozen = json!({"text": tag});
    if hidden {
        frozen["sign"] = json!("hidden");
    }
    let acc: Vec<Value> = accessories
        .iter()
        .zip(1u32..)
        .map(|(p, seq)| json!({"after_seq": seq, "kind": "end_clamp", "part": {"copy": uid(*p)}}))
        .collect();
    fx.push(json!({
        "$k": "device", "id": uid(n), "tag": {"letter": "X", "frozen": frozen},
        "strip": {"accessories": acc},
    }));
}

/// A terminal of `strip_dev` with one part, placed on `page`.
fn terminal(fx: &mut Fx, n: u128, strip_dev: u128, seq: u32, designation: &str, part: u128, page: u128) {
    fx.push(json!({
        "$k": "device", "id": uid(n), "tag": {"letter": "X"},
        "terminal": {"strip": uid(strip_dev), "seq": seq, "designation": designation},
        "parts": part_uses(&[(part, "1")]),
    }));
    place(fx, n, page);
}

fn rows(fx: &Fx, mode: BomMode) -> Vec<BomRow> {
    let m = fx.build();
    let q = Q::new(&m);
    q.get::<BomRows>(mode).as_ref().clone()
}

fn numbers(rows: &[BomRow]) -> Vec<&str> {
    rows.iter().map(|r| r.part_number.as_str()).collect()
}

fn row<'a>(rows: &'a [BomRow], number: &str) -> &'a BomRow {
    rows.iter()
        .find(|r| r.part_number == number)
        .unwrap_or_else(|| panic!("no row {number}"))
}

/// A subset of doc 20 §14.1.
fn golden() -> Fx {
    let mut fx = base();
    // Page 3: a plug.
    dev(
        &mut fx,
        0x1000,
        "3X1",
        false,
        &[(P_PROFIBUS_PLUG, "1")],
        Some(PAGE3),
    );
    // Page 101: two MCBs with different parts.
    dev(&mut fx, 0x1010, "101F1", false, &[(P_MCB_A, "1")], Some(PAGE101));
    dev(&mut fx, 0x1020, "F55", true, &[(P_MCB_B, "1")], Some(PAGE101));
    // Strip 1X1 (hidden sign), two UK 5 N terminals and a PE terminal.
    strip(&mut fx, 0x1100, "1X1", true, &[]);
    terminal(&mut fx, 0x1110, 0x1100, 1, "1", P_UK5N, PAGE101);
    terminal(&mut fx, 0x1120, 0x1100, 2, "2", P_UK5N, PAGE101);
    terminal(&mut fx, 0x1130, 0x1100, 3, "PE", P_USLKG5, PAGE101);
    // Strip -X1031 on page 103: terminals 10 and 2 (the lower ID is the higher designation), two end
    // brackets as strip accessories; the strip has no segment, so it is placed through its first terminal
    // (terminal 2 on page 103: `MainRep` of a strip without a segment).
    strip(&mut fx, 0x1200, "X1031", false, &[P_END_BRACKET, P_END_BRACKET]);
    terminal(&mut fx, 0x1210, 0x1200, 10, "10", P_PT25, PAGE103);
    terminal(&mut fx, 0x1220, 0x1200, 2, "2", P_PT25, PAGE103);
    // 550208 on -201U1 and -202U1.
    dev(
        &mut fx,
        0x2000,
        "201U1",
        false,
        &[(P_LINK_BLOCK, "1")],
        Some(PAGE201),
    );
    dev(
        &mut fx,
        0x2100,
        "202U1",
        false,
        &[(P_LINK_BLOCK, "1")],
        Some(PAGE202),
    );
    // A placed device without parts.
    dev(&mut fx, 0x1300, "F99", false, &[], Some(PAGE101));
    // A device that is not fitted.
    dev(&mut fx, 0x1310, "F98", false, &[(P_SPARE, "1")], Some(PAGE101));
    fx.record(0x1310)["state"] = json!("not_fitted");
    // Unplaced: enclosure, plate, rail (two devices, metres), duct.
    dev(
        &mut fx,
        0x3000,
        "UC1",
        false,
        &[(P_DUCT, "2"), (P_RAIL, "1.5"), (P_PLATE, "1"), (P_ENCLOSURE, "1")],
        None,
    );
    dev(&mut fx, 0x3010, "UC2", false, &[(P_RAIL, "2.25")], None);
    fx
}

#[test]
fn component_rows_are_ordered_and_numbered() {
    let out = rows(&golden(), BomMode::Component);
    assert_eq!(
        numbers(&out),
        [
            "532216",      // page 3
            "5SY4204-7",   // page 101, part number order
            "A9P52610",    //
            "UK 5 N",      //
            "USLKG 5",     //
            "PT 2.5-TWIN", // page 103
            "550208",      // pages 201, 202
            "NSYPLA1074G", // unplaced: enclosure, plate, rail, duct, other
            "NSYPMM107",   //
            "A-RAIL",      //
            "B-DUCT",      //
            "C-EB",        // a layout-only accessory: with the legend rows, pages 103
        ]
    );
    let pos: Vec<u32> = out.iter().map(|r| r.pos).collect();
    assert_eq!(pos, (1..=12).collect::<Vec<u32>>());
    assert!(out.iter().all(|r| !r.configured));
}

#[test]
fn terminals_aggregate_with_devices_in_natural_tag_order() {
    let out = rows(&golden(), BomMode::Component);
    let uk = row(&out, "UK 5 N");
    assert_eq!(uk.qty, Dec3(2_000));
    assert_eq!(uk.devices, [id(0x1110), id(0x1120)]);
    assert_eq!(uk.pages, [PageRef::Drawn(id(PAGE101))]);
    assert_eq!(uk.unit, OrderUnit::Piece);
    assert_eq!(uk.description.as_neutral(), Some("Feed-through terminal"));

    let pe = row(&out, "USLKG 5");
    assert_eq!(pe.qty, Dec3(1_000));
    assert_eq!(pe.devices, [id(0x1130)]);

    // "2" sorts before "10" although its ID is larger.
    let pt = row(&out, "PT 2.5-TWIN");
    assert_eq!(pt.qty, Dec3(2_000));
    assert_eq!(pt.devices, [id(0x1220), id(0x1210)]);
    assert_eq!(pt.pages, [PageRef::Drawn(id(PAGE103))]);
}

#[test]
fn strip_accessories_are_rows_of_the_strip() {
    let m = golden().build();
    let q = Q::new(&m);
    let lines = q.get::<DevicePartLines>(id(0x1200));
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| l.source == LineSource::StripAccessory));
    assert!(lines.iter().all(|l| l.qty == Dec3::ONE && !l.configured));

    let out = rows(&golden(), BomMode::Component);
    let eb = row(&out, "C-EB");
    assert_eq!(eb.qty, Dec3(2_000));
    assert_eq!(eb.devices, [id(0x1200)]);
    // The strip has no segment: it is placed through its first terminal (terminal 2 on page 103).
    assert_eq!(eb.pages, [PageRef::Drawn(id(PAGE103))]);
}

#[test]
fn one_part_on_two_pages_is_one_row() {
    let out = rows(&golden(), BomMode::Component);
    let r = row(&out, "550208");
    assert_eq!(r.qty, Dec3(2_000));
    assert_eq!(r.devices, [id(0x2000), id(0x2100)]);
    assert_eq!(
        r.pages,
        [PageRef::Drawn(id(PAGE201)), PageRef::Drawn(id(PAGE202))]
    );
}

#[test]
fn page_three_sorts_before_page_101() {
    let out = rows(&golden(), BomMode::Component);
    assert_eq!(out[0].part_number, "532216");
    assert_eq!(out[0].pages, [PageRef::Drawn(id(PAGE3))]);
    assert_eq!(out[0].pos, 1);
}

#[test]
fn devices_without_parts_and_not_fitted_devices_have_no_rows() {
    let out = rows(&golden(), BomMode::Component);
    assert!(out.iter().all(|r| r.part_number != "D-SPARE"));
    for r in &out {
        assert!(!r.devices.contains(&id(0x1300)));
        assert!(!r.devices.contains(&id(0x1310)));
    }
    let m = golden().build();
    let q = Q::new(&m);
    assert!(q.get::<DevicePartLines>(id(0x1300)).is_empty());
    assert!(q.get::<DevicePartLines>(id(0x1310)).is_empty());
    assert!(q.get::<DevicePartLines>(id(0x7777)).is_empty());
}

#[test]
fn unplaced_rows_follow_placed_ones_by_legend_rank() {
    let out = rows(&golden(), BomMode::Component);
    let tail: Vec<&str> = numbers(&out)[7..].to_vec();
    assert_eq!(tail, ["NSYPLA1074G", "NSYPMM107", "A-RAIL", "B-DUCT", "C-EB"]);
    assert!(out[7..11].iter().all(|r| r.pages.is_empty()));
    // A layout-only accessory sorts with the legend rows although its strip is placed (doc 20 §14.1 rows
    // 26, 27); it keeps the pages of its devices.
    assert_eq!(out[11].pages, [PageRef::Drawn(id(PAGE103))]);
}

#[test]
fn metre_quantities_add_up_in_dec3() {
    let out = rows(&golden(), BomMode::Component);
    let rail = row(&out, "A-RAIL");
    assert_eq!(rail.qty, Dec3(3_750));
    assert_eq!(rail.unit, OrderUnit::Metre);
    assert_eq!(rail.devices, [id(0x3000), id(0x3010)]);
    assert_eq!(row(&out, "B-DUCT").qty, Dec3(2_000));
}

#[test]
fn an_overflowing_sum_saturates() {
    let mut fx = base();
    dev(
        &mut fx,
        0x1000,
        "F1",
        false,
        &[(P_MCB_A, "9223372036854775.807")],
        Some(PAGE101),
    );
    dev(&mut fx, 0x1010, "F2", false, &[(P_MCB_A, "2")], Some(PAGE101));
    let out = rows(&fx, BomMode::Component);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].qty, Dec3(i64::MAX));
}

#[test]
fn variants_are_separate_rows() {
    let mut fx = base();
    fx.push(json!({
        "$k": "device", "id": uid(0x1000), "tag": {"letter": "F", "frozen": {"text": "F1"}},
        "parts": [
            {"part": {"copy": uid(P_MCB_A), "variant": "b"}, "qty": "1"},
            {"part": {"copy": uid(P_MCB_A), "variant": "a"}, "qty": "2"},
            {"part": {"copy": uid(P_MCB_A)}, "qty": "1"},
        ],
    }));
    place(&mut fx, 0x1000, PAGE101);
    let out = rows(&fx, BomMode::Component);
    assert_eq!(out.len(), 3);
    let by_variant: Vec<(Option<&str>, Dec3)> = out
        .iter()
        .map(|r| (r.key.variant.as_ref().map(|v| v.as_str()), r.qty))
        .collect();
    // Same part number and position: the key (None before Some, then the text) breaks the tie.
    assert_eq!(
        by_variant,
        [
            (None, Dec3(1_000)),
            (Some("a"), Dec3(2_000)),
            (Some("b"), Dec3(1_000))
        ]
    );
}

#[test]
fn lines_follow_the_model_order_and_carry_the_configured_flag() {
    // A reference to a missing part copy cannot occur in a valid snapshot (EW-INV-001); the drop of such
    // a line in `device_part_lines` is defensive.
    let mut fx = base();
    dev(
        &mut fx,
        0x2000,
        "201U1",
        false,
        &[(P_BUS_NODE, "1"), (P_LINK_BLOCK, "1"), (P_CONF_CPX, "1")],
        Some(PAGE201),
    );
    let m = fx.build();
    let q = Q::new(&m);
    let lines = q.get::<DevicePartLines>(id(0x2000));
    let seen: Vec<(Id<ew_schema::Part>, bool)> = lines.iter().map(|l| (l.part, l.configured)).collect();
    assert_eq!(
        seen,
        [
            (id(P_BUS_NODE), false),
            (id(P_LINK_BLOCK), false),
            (id(P_CONF_CPX), true)
        ]
    );
    assert!(
        lines
            .iter()
            .all(|l| l.device == id(0x2000) && l.source == LineSource::Device)
    );
}

/// An assembly config: its configured part (if any) and the devices it covers.
fn config(order_code: &str, part: Option<u128>, covers: &[u128]) -> Value {
    let mut c = json!({"order_code": order_code});
    if let Some(p) = part {
        c["part"] = json!({"copy": uid(p)});
    }
    let mut covers: Vec<String> = covers.iter().map(|d| uid(*d)).collect();
    covers.sort();
    if !covers.is_empty() {
        c["covers"] = json!(covers);
    }
    c
}

/// Head -201U1 (195740 and 550208) with its assembly aspect: order code 1 (the CPX product) covers the head,
/// order code 2 (the MPA product) covers the module -201U3 and its valve -201U3-Q1 (doc 20 §14.2). -202U1
/// is a child that no order code names.
fn assembly_with(configs: Value) -> Fx {
    let mut fx = base();
    dev(
        &mut fx,
        0x2000,
        "201U1",
        false,
        &[(P_BUS_NODE, "1"), (P_LINK_BLOCK, "1")],
        Some(PAGE201),
    );
    fx.record(0x2000)["assembly"] = json!({"configs": configs});
    dev(&mut fx, 0x2010, "201U3", false, &[(P_MODULE, "1")], Some(PAGE201));
    fx.record(0x2010)["parent"] = json!(uid(0x2000));
    // A grandchild: a valve of the module.
    dev(
        &mut fx,
        0x2020,
        "201U3-Q1",
        false,
        &[(P_VALVE, "1")],
        Some(PAGE201),
    );
    fx.record(0x2020)["parent"] = json!(uid(0x2010));
    // -202U1 is a child that no order code covers: its link block stays.
    dev(
        &mut fx,
        0x2100,
        "202U1",
        false,
        &[(P_LINK_BLOCK, "1")],
        Some(PAGE202),
    );
    fx.record(0x2100)["parent"] = json!(uid(0x2000));
    fx
}

fn assembly() -> Fx {
    assembly_with(json!([
        config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000]),
        config("32P-VGL-R", Some(P_CONF_MPA), &[0x2010, 0x2020]),
    ]))
}

#[test]
fn component_mode_ignores_the_assembly() {
    let out = rows(&assembly(), BomMode::Component);
    // All on page 201 at one point: part number order.
    assert_eq!(numbers(&out), ["195740", "533361", "550208", "571334"]);
    assert!(out.iter().all(|r| !r.configured && r.covers.is_empty()));
    let pos: Vec<u32> = out.iter().map(|r| r.pos).collect();
    assert_eq!(pos, [1, 2, 3, 4]);
    assert_eq!(row(&out, "550208").qty, Dec3(2_000));
}

#[test]
fn component_mode_drops_configured_part_uses() {
    let mut fx = base();
    dev(
        &mut fx,
        0x2000,
        "201U1",
        false,
        &[(P_BUS_NODE, "1"), (P_CONF_CPX, "1")],
        Some(PAGE201),
    );
    assert_eq!(numbers(&rows(&fx, BomMode::Component)), ["195740"]);
}

#[test]
fn assembly_mode_replaces_the_covered_rows_by_configured_rows() {
    let out = rows(&assembly(), BomMode::Assembly);
    // Both configured products sit on the head: same position, the config order decides (CPX first), not
    // the part number.
    assert_eq!(numbers(&out), ["51E-F13GCQP-D+H2GG", "32P-VGL-R", "550208"]);
    let configured: Vec<&BomRow> = out.iter().filter(|r| r.configured).collect();
    assert_eq!(configured.len(), 2);
    assert_eq!(configured.iter().map(|r| r.pos).collect::<Vec<_>>(), [1, 2]);
    assert!(
        configured
            .iter()
            .all(|r| r.qty == Dec3::ONE && r.unit == OrderUnit::Set)
    );
    assert!(configured.iter().all(|r| r.devices == [id(0x2000)]));
    assert_eq!(configured[0].covers, [id(0x2000)]);
    assert_eq!(configured[1].covers, [id(0x2010), id(0x2020)]);

    // The head's own components, the module and the valve are gone; -202U1's link block stays (no order
    // code covers it), numbered 1 in the component sequence.
    let rest = row(&out, "550208");
    assert!(!rest.configured);
    assert!(rest.covers.is_empty());
    assert_eq!(rest.pos, 1);
    assert_eq!(rest.qty, Dec3::ONE);
    assert_eq!(rest.devices, [id(0x2100)]);
    assert_eq!(rest.pages, [PageRef::Drawn(id(PAGE202))]);
}

#[test]
fn covers_are_explicit_not_transitive() {
    // Order code 2 names the module but not its valve: the valve keeps its row (one cell for all devices: the
    // head rank orders A2 after the component rows of the cell).
    let fx = assembly_with(json!([
        config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000]),
        config("32P-VGL-R", Some(P_CONF_MPA), &[0x2010]),
    ]));
    let out = rows(&fx, BomMode::Assembly);
    assert_eq!(
        numbers(&out),
        ["51E-F13GCQP-D+H2GG", "571334", "32P-VGL-R", "550208"]
    );
}

#[test]
fn a_config_without_a_part_prints_and_covers_nothing() {
    let fx = assembly_with(json!([
        config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000]),
        config("32P-VGL-R", None, &[0x2010, 0x2020]),
    ]));
    let out = rows(&fx, BomMode::Assembly);
    assert_eq!(
        numbers(&out),
        ["51E-F13GCQP-D+H2GG", "533361", "571334", "550208"]
    );
}

#[test]
fn a_cover_outside_the_heads_subtree_is_ignored() {
    // Order code 2 also names an unrelated device (-101F1, no descendant of -201U1): its part stays.
    let mut fx = assembly_with(json!([
        config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000]),
        config("32P-VGL-R", Some(P_CONF_MPA), &[0x2010, 0x2020, 0x3000]),
    ]));
    dev(&mut fx, 0x3000, "101F1", false, &[(P_MCB_A, "1")], Some(PAGE101));
    let out = rows(&fx, BomMode::Assembly);
    let mcb = row(&out, "A9P52610");
    assert_eq!(mcb.devices, [id(0x3000)]);
    let mpa = row(&out, "32P-VGL-R");
    assert_eq!(
        mpa.covers,
        [id(0x2010), id(0x2020)],
        "the outside cover is not listed"
    );
}

#[test]
fn a_head_covered_by_another_assembly_prints_no_config_rows() {
    // The module -201U3 is itself an assembly head (order code X, covering its valve); -201U1's order code 2
    // covers it, so the outer assembly replaces it: no row X, and the valve is covered by -201U1 only.
    let mut fx = assembly();
    fx.record(0x2010)["assembly"] = json!({"configs": [config("X-INNER", Some(P_SPARE), &[0x2020])]});
    let out = rows(&fx, BomMode::Assembly);
    assert_eq!(numbers(&out), ["51E-F13GCQP-D+H2GG", "32P-VGL-R", "550208"]);
    assert!(out.iter().all(|r| r.part_number != "D-SPARE"));
    // Without the outer cover the inner head prints its row.
    let mut fx = assembly_with(json!([config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000])]));
    fx.record(0x2010)["assembly"] = json!({"configs": [config("X-INNER", Some(P_SPARE), &[0x2020])]});
    let out = rows(&fx, BomMode::Assembly);
    assert!(out.iter().any(|r| r.part_number == "D-SPARE" && r.configured));
}

#[test]
fn a_components_basis_keeps_the_components_in_assembly_mode() {
    let mut fx = assembly();
    fx.record(0x2000)["assembly"]["bom_basis"] = json!("components");
    let assembly_rows = rows(&fx, BomMode::Assembly);
    assert_eq!(numbers(&assembly_rows), numbers(&rows(&fx, BomMode::Component)));
    assert!(assembly_rows.iter().all(|r| !r.configured));
}

#[test]
fn a_configured_part_use_without_an_aspect_stays_and_covers_nothing() {
    let mut fx = base();
    dev(
        &mut fx,
        0x2000,
        "201U1",
        false,
        &[(P_BUS_NODE, "1"), (P_CONF_CPX, "1")],
        Some(PAGE201),
    );
    let out = rows(&fx, BomMode::Assembly);
    // A part no config names ranks after the configs: after the component row of the cell.
    assert_eq!(numbers(&out), ["195740", "51E-F13GCQP-D+H2GG"]);
    assert!(out[1].configured && out[1].covers.is_empty());
}

#[test]
fn configured_rows_follow_the_assembly_config_order() {
    // The part numbers sort 32P-... < 51E-...; the config order decides either way.
    let cpx_first = assembly();
    assert_eq!(
        numbers(&rows(&cpx_first, BomMode::Assembly))[..2],
        ["51E-F13GCQP-D+H2GG", "32P-VGL-R"]
    );
    let mpa_first = assembly_with(json!([
        config("32P-VGL-R", Some(P_CONF_MPA), &[0x2010, 0x2020]),
        config("51E-F13GCQP-D+H2GG", Some(P_CONF_CPX), &[0x2000]),
    ]));
    assert_eq!(
        numbers(&rows(&mpa_first, BomMode::Assembly))[..2],
        ["32P-VGL-R", "51E-F13GCQP-D+H2GG"]
    );
}

// WP-S2.07 part 1 (D6, SE4)
/// A bridge connection `n` of `group` between CP 0 of the main functions of terminals `a` and `b` (function = device
/// + 1), with `part` on `overrides.wire_part` when given.
fn bridge(fx: &mut Fx, n: u128, group: u16, a: u128, b: u128, part: Option<u128>) {
    let mut r = json!({
        "$k": "connection", "id": uid(n),
        "a": {"cp": [uid(a + 1), 0]}, "b": {"cp": [uid(b + 1), 0]},
        "kind": {"bridge": {"kind": "plug_in", "group": group, "shaft": 1}},
    });
    if let Some(p) = part {
        r["overrides"] = json!({"wire_part": {"copy": uid(p)}});
    }
    fx.push(r);
}

/// A strip with four terminals: BR group 1 over :1 to :3 (two connections, the part on the second, which has the
/// smaller ID but the higher sequence), group 2 over :3 and :4 without a part.
fn bridged() -> Fx {
    let mut fx = base();
    strip(&mut fx, 0x4000, "X9", false, &[]);
    for (k, seq) in [(0x4010, 1), (0x4020, 2), (0x4030, 3), (0x4040, 4)] {
        terminal(&mut fx, k, 0x4000, seq, &seq.to_string(), P_UK5N, PAGE103);
    }
    // Connection IDs chosen against the (seq, ID) order: 2-3 has the smaller ID.
    bridge(&mut fx, 0x4102, 1, 0x4010, 0x4020, None);
    bridge(&mut fx, 0x4101, 1, 0x4020, 0x4030, Some(P_SPARE));
    bridge(&mut fx, 0x4103, 2, 0x4030, 0x4040, None);
    fx
}

#[test]
fn strip_bridges_group_the_connections_and_name_the_carrier() {
    use ew_derive::StripBridges;
    let m = bridged().build();
    let q = Q::new(&m);
    let groups = q.get::<StripBridges>(id(0x4000));
    assert_eq!(groups.len(), 2);
    let g1 = &groups[0];
    assert_eq!(g1.group, 1);
    assert_eq!(g1.terminals, [id(0x4010), id(0x4020), id(0x4030)]);
    // (lower seq, ID) order: 1-2 (seq 1) before 2-3 (seq 2) although 2-3 has the smaller ID.
    assert_eq!(g1.connections, [id(0x4102), id(0x4101)]);
    // D6: the first connection with a part is the carrier.
    assert_eq!(g1.carrier, id(0x4101));
    assert_eq!(g1.part.as_ref().map(|p| p.copy), Some(id(P_SPARE)));
    let g2 = &groups[1];
    assert_eq!(g2.group, 2);
    assert_eq!(g2.carrier, id(0x4103));
    assert!(g2.part.is_none());
    // A terminal is not a strip: it has no bridges of its own.
    assert!(q.get::<StripBridges>(id(0x4010)).is_empty());

    // One Bridge line, for group 1 only.
    let lines = q.get::<DevicePartLines>(id(0x4000));
    let bridge_lines: Vec<_> = lines.iter().filter(|l| l.source == LineSource::Bridge).collect();
    assert_eq!(bridge_lines.len(), 1);
    assert_eq!(bridge_lines[0].part, id(P_SPARE));
    assert_eq!(bridge_lines[0].qty, Dec3::ONE);

    // The BOM counts it once, on the strip.
    let out = rows(&bridged(), BomMode::Component);
    let spare = row(&out, "D-SPARE");
    assert_eq!(spare.qty, Dec3(1_000));
    assert_eq!(spare.devices, [id(0x4000)]);
}

#[test]
fn a_bridge_without_any_part_takes_its_first_connection_as_carrier() {
    use ew_derive::StripBridges;
    let mut fx = bridged();
    if let Some(o) = fx.record(0x4101).as_object_mut() {
        o.remove("overrides");
    }
    let m = fx.build();
    let q = Q::new(&m);
    let groups = q.get::<StripBridges>(id(0x4000));
    assert_eq!(groups[0].carrier, id(0x4102));
    assert!(groups[0].part.is_none());
    assert!(
        q.get::<DevicePartLines>(id(0x4000))
            .iter()
            .all(|l| l.source != LineSource::Bridge)
    );
}
