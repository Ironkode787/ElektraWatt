// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S4.01 part 4 on a small fixture: the carrier lines of the enclosure device (D11, the orchestrator's rulings 5 and
//! 6 of cycle 41), the folios of the layout objects' BOM rows (FT-204), `layout_legend(surface)` with its order and
//! renumbering (LB-2's row content), and a free graphic counted nowhere (LB-4). Golden A's values are `ew-api`'s
//! `tests/golden_layout.rs`.

mod common;

use common::{Fx, LIBRARY, id, uid};
use ew_derive::layout::{LegendItem, LegendRow, layout_legend, metres_of_nm, page_surface, surface_pages};
use ew_derive::{BomMode, BomRow, BomRows, DevicePartLines, DeviceTag, LineSource, PageRef, Q};
use ew_ids::Id;
use ew_mltext::natural_key;
use ew_schema::{Device, Part, PartRef};
use ew_units::Dec3;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

const LOC: u128 = 0x10;
const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const P_ENC: u128 = 0x600;
const P_PLATE: u128 = 0x601;
const P_RAIL: u128 = 0x602;
const P_DUCT: u128 = 0x603;
const P_DUCT_B: u128 = 0x604; // only on the loaded diagonal duct
const P_MCB: u128 = 0x605;
const P_LAMP: u128 = 0x606; // a second part use of the enclosure device

const D_ENC: u128 = 0xd0; // -U1, owns the enclosure
const D_PLATE: u128 = 0xd1; // -U1-plate, the surface's device
const D_F1: u128 = 0xd2; // -F1, placed on R_A
const D_F2: u128 = 0xd3; // -F2, placed on R_A, drawn on the schematic page
const D_F3: u128 = 0xd4; // -F3, unplaced (the renumbering case places it)

const PAGE_SCHEMATIC: u128 = 0x31;
const PAGE_PANEL: u128 = 0x32;
const VIEW: u128 = 0x40;
const GRAPHIC: u128 = 0x41;

const SPACE: u128 = 0x20;
const ENCLOSURE: u128 = 0x21;
const SURFACE: u128 = 0x22;
const R_A: u128 = 0x23; // rail (0, 700) → (500, 700), 500 mm
const R_B: u128 = 0x24; // rail (0, 300) → (500, 300), 500 mm
const D_A: u128 = 0x25; // duct (0, 900) → (500, 900), 500 mm
const D_B: u128 = 0x26; // duct (20, 850) → (20, 0), 850 mm
const D_DIAG: u128 = 0x27; // duct (100, 100) → (200, 200): breaks D5 (loaded only), no length
const R_NOPART: u128 = 0x28; // rail (0, 100) → (300, 100), no part
const PL_F2: u128 = 0x52;
const PL_F1: u128 = 0x51;
const PL_F3: u128 = 0x53;

fn mm(v: i64) -> i64 {
    v * MM
}

fn libref(n: u128) -> Value {
    json!({"library": uid(LIBRARY), "item": uid(n + 0x1_0000), "version_req": "^1"})
}

fn part(fx: &mut Fx, n: u128, number: &str, kind: &str, unit: &str, extra: Value) {
    let mut item = json!({
        "meta": {}, "kind": kind, "part_number": number, "description": number, "unit": unit,
        "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
    });
    for (k, v) in extra.as_object().expect("an object") {
        item[k] = v.clone();
    }
    fx.lib_item("part", n, item);
}

fn device(fx: &mut Fx, n: u128, letter: &str, counter: u32, parts: &[u128]) {
    fx.device(n, letter, Some(counter))
        .with("structure", json!({"location": uid(LOC)}));
    let uses: Vec<Value> = parts
        .iter()
        .map(|p| json!({"part": {"copy": uid(*p)}, "qty": "1"}))
        .collect();
    fx.with("parts", json!(uses));
}

fn carrier(fx: &mut Fx, n: u128, kind: &str, part: Option<u128>, path: &[(i64, i64)]) {
    let path: Vec<Value> = path.iter().map(|(u, v)| json!([mm(*u), mm(*v)])).collect();
    let params = if kind == "rail" {
        json!({"rail": {"profile": "ts35x7_5"}})
    } else {
        json!({"duct": {"width": mm(40), "height": mm(60)}})
    };
    let mut r = json!({"$k": "carrier", "id": uid(n), "surface": uid(SURFACE), "kind": kind, "path": path,
                       "params": params});
    if let Some(p) = part {
        r["part"] = json!({"copy": uid(p)});
    }
    fx.push(r);
}

fn place(fx: &mut Fx, n: u128, device: u128, s: i64) {
    fx.push(
        json!({"$k": "placement", "id": uid(n), "device": uid(device), "carrier": {"carrier": uid(R_A)},
                   "pose": {"t": [mm(s), 0, 0], "q": [1_000_000_000, 0, 0, 0]}}),
    );
}

/// The fixture: -U1 with an enclosure part (and a lamp), its plate -U1-plate, four carriers with parts (two rails of
/// one part, two ducts of another), a diagonal duct of a third part (loaded data, no length), a rail without a part,
/// -F1 and -F2 on rail R_A, -F3 unplaced; a schematic page drawing -F2 and the panel page viewing the surface.
fn fixture() -> Fx {
    let mut fx = Fx::new();
    fx.node(LOC, "location", "A", None)
        .page(PAGE_SCHEMATIC, 3, None, "multi_line", &[], "Schematic")
        .page(PAGE_PANEL, 902, None, "panel_layout", &[], "Cabinet Layout")
        .fndef(
            FNDEF,
            ["Protection", "Overcurrent", "CircuitBreaker"],
            &[(0, "1"), (1, "2")],
        )
        .symbol(
            SYM,
            FNDEF,
            json!("origin"),
            &[(0, 0, 0, 0), (1, 1, 0, 16_000_000)],
        );
    part(
        &mut fx,
        P_ENC,
        "ENC",
        "enclosure",
        "piece",
        json!({"sections": {"enclosure": {"width": mm(600), "height": mm(1000), "plate": libref(P_PLATE)}}}),
    );
    part(
        &mut fx,
        P_PLATE,
        "PLATE",
        "mounting_plate",
        "piece",
        json!({"sections": {"mounting_plate": {"width": mm(500), "height": mm(900)}}}),
    );
    part(&mut fx, P_RAIL, "RAIL", "rail", "m", json!({}));
    part(&mut fx, P_DUCT, "DUCT", "duct", "m", json!({}));
    part(&mut fx, P_DUCT_B, "DUCT-B", "duct", "m", json!({}));
    part(
        &mut fx,
        P_MCB,
        "MCB",
        "device",
        "piece",
        json!({"mech": {"width": mm(18), "height": mm(90), "mount_kind": "ts35x7_5"}}),
    );
    part(&mut fx, P_LAMP, "LAMP", "device", "piece", json!({}));
    device(&mut fx, D_ENC, "U", 1, &[P_LAMP, P_ENC]);
    device(&mut fx, D_PLATE, "U", 2, &[P_PLATE]);
    for (n, c) in [(D_F1, 1), (D_F2, 2), (D_F3, 3)] {
        device(&mut fx, n, "F", c, &[P_MCB]);
        fx.function(n + 0x1000, Some(n), FNDEF, true);
    }
    fx.rep(
        0x900,
        PAGE_SCHEMATIC,
        SYM,
        &[D_F2 + 0x1000],
        40_000_000,
        40_000_000,
    );
    let identity = json!({"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]});
    fx.push(json!({"$k": "layout_space", "id": uid(SPACE), "location": uid(LOC)}))
        .push(
            json!({"$k": "enclosure", "id": uid(ENCLOSURE), "space": uid(SPACE), "device": uid(D_ENC),
                     "pose": identity}),
        )
        .push(
            json!({"$k": "mounting_surface", "id": uid(SURFACE), "enclosure": uid(ENCLOSURE), "kind": "plate",
                     "device": uid(D_PLATE), "frame": identity}),
        );
    carrier(&mut fx, D_B, "duct", Some(P_DUCT), &[(20, 850), (20, 0)]);
    carrier(&mut fx, R_B, "rail", Some(P_RAIL), &[(0, 300), (500, 300)]);
    carrier(&mut fx, D_DIAG, "duct", Some(P_DUCT_B), &[(100, 100), (200, 200)]);
    carrier(&mut fx, R_A, "rail", Some(P_RAIL), &[(0, 700), (500, 700)]);
    carrier(&mut fx, D_A, "duct", Some(P_DUCT), &[(0, 900), (500, 900)]);
    carrier(&mut fx, R_NOPART, "rail", None, &[(0, 100), (300, 100)]);
    place(&mut fx, PL_F2, D_F2, 100);
    place(&mut fx, PL_F1, D_F1, 200);
    fx.push(
        json!({"$k": "page_item", "id": uid(VIEW), "page": uid(PAGE_PANEL),
                   "kind": {"surface_view": {"surface": uid(SURFACE), "datum_at": [mm(25), mm(167)]}}}),
    );
    fx
}

fn lines(q: &Q, device: u128) -> Vec<(Id<Part>, Dec3, LineSource)> {
    q.get::<DevicePartLines>(id(device))
        .iter()
        .map(|l| (l.part, l.qty, l.source))
        .collect()
}

fn m(text: &str) -> Dec3 {
    text.parse().expect("a Dec3")
}

fn row_of(rows: &[BomRow], part: u128) -> BomRow {
    rows.iter()
        .find(|r| r.key.part == id(part))
        .cloned()
        .unwrap_or_else(|| panic!("no BOM row of part {part:#x}"))
}

/// Ruling 5 (cycle 41): "the carriers of one enclosure device that share a part copy give one part line
/// (`LineSource::Carrier`), whose quantity is the sum of their path lengths in metres, exact to the millimetre (`Dec3`).
/// The lines come after the device's authored part uses." Ruling 6: "A carrier without a length adds nothing to its
/// line's quantity. The line is never dropped: if every carrier of that part lacks a length, the line stays with
/// quantity 0."
#[test]
fn the_enclosure_device_owns_one_carrier_line_per_part_after_its_own_lines() {
    let model = fixture().build();
    let q = Q::new(&model);
    assert_eq!(
        lines(&q, D_ENC),
        vec![
            (id(P_LAMP), Dec3::ONE, LineSource::Device),
            (id(P_ENC), Dec3::ONE, LineSource::Device),
            // R_A and R_B: 500 + 500 mm (the rail without a part gives nothing).
            (id(P_RAIL), m("1"), LineSource::Carrier),
            // D_A and D_B: 500 + 850 mm.
            (id(P_DUCT), m("1.35"), LineSource::Carrier),
            // The diagonal duct has no length (ruling 6): its line stays, at 0.
            (id(P_DUCT_B), Dec3::ZERO, LineSource::Carrier),
        ]
    );
    // The surface's device and the placed devices own no carrier line.
    assert_eq!(
        lines(&q, D_PLATE),
        vec![(id(P_PLATE), Dec3::ONE, LineSource::Device)]
    );
    assert_eq!(lines(&q, D_F1), vec![(id(P_MCB), Dec3::ONE, LineSource::Device)]);
}

/// Ruling 6 again, with a length that is not whole millimetres: the carriers sum in nanometres and round once.
#[test]
fn carrier_lengths_sum_in_nanometres_and_round_once_to_the_millimetre() {
    let mut fx = fixture();
    // R_A 500.4 mm and R_B 500.4 mm: 1000.8 mm, so 1.001 m (two roundings would give 1 m).
    fx.record(R_A)["path"] = json!([[0, mm(700)], [mm(500) + 400_000, mm(700)]]);
    fx.record(R_B)["path"] = json!([[0, mm(300)], [mm(500) + 400_000, mm(300)]]);
    let model = fx.build();
    let q = Q::new(&model);
    assert_eq!(lines(&q, D_ENC)[2], (id(P_RAIL), m("1.001"), LineSource::Carrier));
    assert_eq!(metres_of_nm(1_499_999), Dec3(1));
    assert_eq!(metres_of_nm(1_500_000), Dec3(2));
    assert_eq!(metres_of_nm(0), Dec3::ZERO);
    assert_eq!(metres_of_nm(i128::MAX), Dec3(i64::MAX), "saturates");
}

/// FT-204, ruling 5: "layout objects print the folios of the panel pages that show their surface" in the BOM's Folios
/// cell; a placed device's own row keeps its representation folios.
#[test]
fn layout_object_rows_print_the_panel_page_and_placed_devices_keep_their_own() {
    let model = fixture().build();
    let q = Q::new(&model);
    assert_eq!(page_surface(&q, id(PAGE_PANEL)), Some(id(SURFACE)));
    assert_eq!(page_surface(&q, id(PAGE_SCHEMATIC)), None);
    assert_eq!(surface_pages(&q, id(SURFACE)), vec![id(PAGE_PANEL)]);
    let rows = q.get::<BomRows>(BomMode::Component);
    let panel = vec![PageRef::Drawn(id(PAGE_PANEL))];
    for p in [P_ENC, P_LAMP, P_PLATE, P_RAIL, P_DUCT, P_DUCT_B] {
        assert_eq!(row_of(&rows, p).pages, panel, "part {p:#x}");
    }
    // -F1 and -F2 are placed on the plate, but their row prints where -F2 is drawn.
    assert_eq!(
        row_of(&rows, P_MCB).pages,
        vec![PageRef::Drawn(id(PAGE_SCHEMATIC))]
    );
    assert_eq!(row_of(&rows, P_RAIL).qty, m("1"));
    assert_eq!(row_of(&rows, P_RAIL).devices, vec![id::<Device>(D_ENC)]);
    // Rows without a placed device keep the legend order: enclosure, plate, rail, duct, then the others.
    let order: Vec<Id<Part>> = rows.iter().map(|r| r.key.part).collect();
    assert_eq!(
        order,
        vec![
            id(P_MCB),
            id(P_ENC),
            id(P_PLATE),
            id(P_RAIL),
            id(P_DUCT),
            id(P_DUCT_B),
            id(P_LAMP)
        ]
    );

    // Without the surface view no page shows the surface: the layout rows print no folio.
    let mut fx = fixture();
    fx.records.retain(|r| r["id"] != json!(uid(VIEW)));
    let model = fx.build();
    let q = Q::new(&model);
    let rows = q.get::<BomRows>(BomMode::Component);
    for p in [P_ENC, P_PLATE, P_RAIL, P_DUCT] {
        assert!(row_of(&rows, p).pages.is_empty(), "part {p:#x}");
    }
}

fn legend(q: &Q) -> Vec<LegendRow> {
    layout_legend(q, id(SURFACE))
}

fn row(pos: u32, item: LegendItem, device: Option<u128>, part: Option<u128>, qty: Option<&str>) -> LegendRow {
    LegendRow {
        pos,
        item,
        device: device.map(id),
        part: part.map(|p| PartRef::new(id(p))),
        qty: qty.map(m),
    }
}

/// D11's legend order: the enclosure, the plate, one row per carrier (rails, then ducts, each top to bottom then left
/// to right by `path[0]`, the DT empty, the length as quantity), then the placements by DT; positions 1 … n.
#[test]
fn the_legend_lists_enclosure_plate_carriers_then_placements_by_dt() {
    let model = fixture().build();
    let q = Q::new(&model);
    use LegendItem::{Carrier, Enclosure, Placement, Surface};
    assert_eq!(
        legend(&q),
        vec![
            row(1, Enclosure(id(ENCLOSURE)), Some(D_ENC), Some(P_ENC), Some("1")),
            row(2, Surface(id(SURFACE)), Some(D_PLATE), Some(P_PLATE), Some("1")),
            row(3, Carrier(id(R_A)), None, Some(P_RAIL), Some("0.5")),
            row(4, Carrier(id(R_B)), None, Some(P_RAIL), Some("0.5")),
            row(5, Carrier(id(R_NOPART)), None, None, Some("0.3")),
            row(6, Carrier(id(D_A)), None, Some(P_DUCT), Some("0.5")),
            row(7, Carrier(id(D_B)), None, Some(P_DUCT), Some("0.85")),
            row(8, Carrier(id(D_DIAG)), None, Some(P_DUCT_B), None),
            row(9, Placement(id(PL_F1)), Some(D_F1), Some(P_MCB), Some("1")),
            row(10, Placement(id(PL_F2)), Some(D_F2), Some(P_MCB), Some("1")),
        ]
    );
    assert!(layout_legend(&q, id(0x9999)).is_empty(), "a missing surface");
}

/// LB-2's renumbering: placing a device adds one row at its DT and renumbers the rows after it.
#[test]
fn placing_a_device_adds_its_row_and_renumbers_the_rows_after_it() {
    let before = {
        let model = fixture().build();
        legend(&Q::new(&model))
    };
    let mut fx = fixture();
    place(&mut fx, PL_F3, D_F3, 300);
    let model = fx.build();
    let q = Q::new(&model);
    let after = legend(&q);
    assert_eq!(after.len(), before.len() + 1);
    let positions: Vec<u32> = after.iter().map(|r| r.pos).collect();
    assert_eq!(positions, (1..=11).collect::<Vec<u32>>());
    // The placement rows are in natural order of the full designation.
    let full = |d: Id<Device>| {
        let tag = q.get::<DeviceTag>(d);
        natural_key(tag.as_ref().as_ref().map_or("", |t| t.full.as_str()))
    };
    let devices: Vec<Id<Device>> = after[8..].iter().filter_map(|r| r.device).collect();
    let mut sorted = devices.clone();
    sorted.sort_by_key(|d| (full(*d), *d));
    assert_eq!(devices, sorted);
    // The new row sits at -F3's designation, not at the end, and every row after it moved down by one.
    let k = after
        .iter()
        .position(|r| r.device == Some(id(D_F3)))
        .expect("-F3 is listed");
    assert!(k < after.len() - 1, "-F3 sorts before another placement");
    for (i, b) in before.iter().enumerate() {
        let a = &after[if i < k { i } else { i + 1 }];
        assert_eq!((&a.item, a.pos), (&b.item, b.pos + u32::from(i >= k)), "row {i}");
    }
}

/// LB-4 (FR-LAY-008): a free graphic on the panel page appears in neither the BOM nor the legend nor `unplaced`.
#[test]
fn a_free_graphic_on_the_panel_page_is_counted_nowhere() {
    let plain = fixture().build();
    let mut fx = fixture();
    fx.push(
        json!({"$k": "page_item", "id": uid(GRAPHIC), "page": uid(PAGE_PANEL),
                   "kind": {"graphic": {"shape": {"circle": {"centre": [mm(30), mm(90)], "r": mm(5)}}}}}),
    );
    let with = fx.build();
    let (a, b) = (Q::new(&plain), Q::new(&with));
    for mode in [BomMode::Component, BomMode::Assembly] {
        assert_eq!(*a.get::<BomRows>(mode), *b.get::<BomRows>(mode));
    }
    assert_eq!(legend(&a), legend(&b));
    assert_eq!(
        *a.get::<ew_derive::layout::Unplaced>(id(SPACE)),
        *b.get::<ew_derive::layout::Unplaced>(id(SPACE))
    );
    assert_eq!(
        *b.get::<ew_derive::layout::Unplaced>(id(SPACE)),
        vec![id::<Device>(D_F3)]
    );
}
