// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `enclosure_legend` at the row level (WP-S4.01 part 4, D11; the orchestrator's ruling 4 of cycle 41: "registered in
//! `ew-reports` with rows from that query, tested at the row level and placed on no page"): the cells of each row
//! kind, a carrier without a derivable length, and the blocks that have no surface. Golden A's 30 rows are `ew-api`'s
//! `tests/golden_layout_bom.rs`.

use ew_derive::Q;
use ew_derive::ew_ids::Uuid;
use ew_derive::ew_schema::{LibDocument, Project, Record, ReportBlock, ReportKind};
use ew_model::Model;
use ew_reports::{Cell, MissingReason, Registry, RowKey, RowSet};
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

fn uid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

const LIBRARY: u128 = 0x7000;
const P_ENC: u128 = 0x600;
const P_RAIL: u128 = 0x601;
const D_ENC: u128 = 0xd0;
const PANEL: u128 = 0x31;
const SCHEMATIC: u128 = 0x32;
const SURFACE: u128 = 0x22;
const RAIL: u128 = 0x23;
const DIAGONAL: u128 = 0x24;

fn part(n: u128, number: &str, kind: &str, unit: &str) -> LibDocument {
    serde_json::from_value(json!({
        "$k": "part", "id": uid(n), "library_id": uid(LIBRARY), "item_id": uid(n + 0x1_0000), "version": "1.0.0",
        "sha256": format!("sha256:{}", "ab".repeat(32)), "meta": {}, "kind": kind, "part_number": number,
        "description": number, "unit": unit,
        "manufacturer": {"library": uid(LIBRARY), "item": uid(0x9000), "version_req": "^1"},
    }))
    .expect("a part copy")
}

fn page(n: u128, folio: u32, kind: &str) -> Value {
    json!({"$k": "page", "id": uid(n), "folio": {"number": folio}, "kind": kind, "title": {"en": "Page"},
           "grid": {"num": 4_000_000, "den": 1}})
}

/// A plate with a 500 mm rail of a part and a diagonal duct without a part (loaded data: no length), viewed by the panel
/// page; the enclosure device carries the enclosure part.
fn model() -> Model {
    let identity = json!({"t": [0, 0, 0], "q": [1_000_000_000, 0, 0, 0]});
    let records: Vec<Value> = vec![
        json!({"$k": "structure_node", "id": uid(0x10), "aspect": "location", "code": "A"}),
        page(PANEL, 902, "panel_layout"),
        page(SCHEMATIC, 3, "multi_line"),
        json!({"$k": "device", "id": uid(D_ENC), "tag": {"letter": "U", "counter": 1},
               "structure": {"location": uid(0x10)}, "parts": [{"part": {"copy": uid(P_ENC)}, "qty": "1"}]}),
        json!({"$k": "layout_space", "id": uid(0x20), "location": uid(0x10)}),
        json!({"$k": "enclosure", "id": uid(0x21), "space": uid(0x20), "device": uid(D_ENC), "pose": identity}),
        json!({"$k": "mounting_surface", "id": uid(SURFACE), "enclosure": uid(0x21), "kind": "plate",
               "frame": identity}),
        json!({"$k": "carrier", "id": uid(RAIL), "surface": uid(SURFACE), "kind": "rail", "part": {"copy": uid(P_RAIL)},
               "path": [[0, 500 * MM], [500 * MM, 500 * MM]], "params": {"rail": {"profile": "ts35x7_5"}}}),
        json!({"$k": "carrier", "id": uid(DIAGONAL), "surface": uid(SURFACE), "kind": "duct",
               "path": [[0, 0], [100 * MM, 100 * MM]], "params": {"duct": {"width": 40 * MM, "height": 60 * MM}}}),
        json!({"$k": "page_item", "id": uid(0x40), "page": uid(PANEL),
               "kind": {"surface_view": {"surface": uid(SURFACE), "datum_at": [25 * MM, 167 * MM]}}}),
    ];
    let project: Project =
        serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).expect("a project");
    let records: Vec<Record> = records
        .into_iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}")))
        .collect();
    let lib = vec![
        part(P_ENC, "ENC", "enclosure", "piece"),
        part(P_RAIL, "RAIL", "rail", "piece"),
    ];
    Model::from_records(project, records, lib).expect("a valid model")
}

/// A legend block, embedded on `page` when given.
fn block(page: Option<u128>) -> ReportBlock {
    let mut b = json!({"id": uid(0x50), "source": {"kind": "enclosure_legend"}, "form": uid(0x51)});
    if let Some(p) = page {
        b["embedded"] = json!({"page": uid(p), "rect": [[0, 0], [100 * MM, 100 * MM]]});
    }
    serde_json::from_value(b).expect("a report block")
}

fn rows(q: &Q, page: Option<u128>) -> RowSet {
    Registry::builtin()
        .get(&ReportKind::EnclosureLegend)
        .expect("enclosure_legend is registered")
        .rows(q, &block(page))
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Ident(s) => s.clone(),
        Cell::Text(t) => t.as_neutral().unwrap_or_default().to_owned(),
        Cell::Qty(q, _) => q.to_string(),
        Cell::Tag(_) => "<tag>".to_owned(),
        Cell::Empty => String::new(),
        Cell::Missing(MissingReason::Length) => "<length missing>".to_owned(),
        other => format!("{other:?}"),
    }
}

#[test]
fn the_rows_of_the_plate_cell_by_cell() {
    let m = model();
    let q = Q::new(&m);
    let set = rows(&q, Some(PANEL));
    let keys: Vec<&str> = set.columns.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "pos",
            "tag",
            "manufacturer",
            "part_number",
            "description",
            "qty",
            "unit"
        ]
    );
    let cells: Vec<Vec<String>> = set
        .rows
        .iter()
        .map(|r| r.cells.iter().map(text).collect())
        .collect();
    let row = |v: [&str; 7]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    // The manufacturer copy is not embedded: the cell prints the reference's item ID.
    let maker = uid(0x9000);
    assert_eq!(
        cells,
        vec![
            row(["1", "<tag>", &maker, "ENC", "ENC", "1", "pc"]),
            // A surface without a device and an enclosure part without a plate: no part.
            row(["2", "", "", "", "", "1", "pc"]),
            // A carrier's quantity is its length, in metres whatever the part's order unit.
            row(["3", "", &maker, "RAIL", "RAIL", "0.5", "m"]),
            // Ruling 6: a carrier without a derivable length prints the length as missing.
            row(["4", "", "", "", "", "<length missing>", "m"]),
        ]
    );
    let ids: Vec<RowKey> = set.rows.iter().map(|r| r.id.0[0].clone()).collect();
    let entity = |n: u128| RowKey::Entity(Uuid::from_u128(n));
    assert_eq!(
        ids,
        vec![entity(0x21), entity(SURFACE), entity(RAIL), entity(DIAGONAL)]
    );
}

#[test]
fn a_block_without_a_viewed_surface_has_no_rows() {
    let m = model();
    let q = Q::new(&m);
    assert!(rows(&q, None).rows.is_empty(), "not embedded");
    assert!(
        rows(&q, Some(SCHEMATIC)).rows.is_empty(),
        "a page without a surface view"
    );
    assert!(rows(&q, Some(0x9999)).rows.is_empty(), "a missing page");
}
