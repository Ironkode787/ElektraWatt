// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The numbering report (FR-REP-022, doc 09 §2.4) at the row level, on a small project of its own: four relays of the
//! child scheme, so that each one's rule value is its letter and counter (`K1` … `K4`), three of them with a frozen tag
//! (`K1` as the rule gives it, `K10` and `K9` against it), and two wires, one with the frozen number `W5`.

use ew_derive::Q;
use ew_ids::Uuid;
use ew_schema::{LibDocument, Project, Record, ReportBlock};
use ew_model::Model;
use ew_reports::Registry;
use ew_reports::csv::row_texts;
use serde_json::{Value, json};

fn uid(n: u128) -> String {
    Uuid::from_u128(n).hyphenated().to_string()
}

const LIBRARY: u128 = 0x7000;
const PAGE: u128 = 0x100;
const DEF: u128 = 0x500;
const WIRE_FROZEN: u128 = 0x2000;

/// Relay `-K<counter>` (child scheme, no parent: its rule value is `K<counter>`), frozen to `frozen` if given, with
/// one function of two connection points.
fn relay(counter: u128, frozen: Option<&str>) -> [Value; 2] {
    let mut tag = json!({"letter": "K", "counter": counter, "scheme": "child"});
    if let Some(f) = frozen {
        tag["frozen"] = json!({"text": f});
    }
    let id = 0x1000 + counter * 0x10;
    [
        json!({"$k": "device", "id": uid(id), "tag": tag}),
        json!({"$k": "function", "id": uid(id + 1), "device": uid(id), "def": uid(DEF)}),
    ]
}

fn model() -> Model {
    let lib: Vec<LibDocument> = vec![
        serde_json::from_value(json!({
            "$k": "fndef", "id": uid(DEF), "library_id": uid(LIBRARY), "item_id": uid(DEF + 0x1_0000),
            "version": "1.0.0", "sha256": format!("sha256:{}", "ab".repeat(32)), "meta": {},
            "category": ["Misc", "Misc", "Plain"], "contact_kind": "black_box",
            "cps": [{"idx": 0, "designation": "1"}, {"idx": 1, "designation": "2"}],
        }))
        .expect("a function definition"),
    ];
    let mut records = vec![json!({
        "$k": "page", "id": uid(PAGE), "folio": {"number": 1}, "kind": "multi_line", "title": {"en": "Page"},
        "grid": {"num": 4_000_000, "den": 1},
    })];
    for (counter, frozen) in [(1, Some("K1")), (2, Some("K10")), (3, Some("K9")), (4, None)] {
        records.extend(relay(counter, frozen));
    }
    let cp = |counter: u128| json!({"cp": [uid(0x1000 + counter * 0x10 + 1), 0]});
    records.push(
        json!({"$k": "connection", "id": uid(WIRE_FROZEN), "page": uid(PAGE), "a": cp(1), "b": cp(2),
                        "number": {"frozen": "W5"}}),
    );
    records.push(json!({"$k": "connection", "id": uid(0x2001), "page": uid(PAGE), "a": cp(3), "b": cp(4)}));
    let project: Project =
        serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).expect("a project");
    let records: Vec<Record> = records
        .into_iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}")))
        .collect();
    Model::from_records(project, records, lib).expect("a valid model")
}

/// A row per frozen tag in the natural order of the tags (`K1`, `K9`, `K10`) with the value the child scheme gives the
/// device and whether the frozen tag matches it, then a row per frozen wire number with its kind and no item; a device
/// and a wire without a frozen value have no row. The frozen `W5` is no number the scheme gives (`<page>.<counter>`),
/// so it deviates.
#[test]
fn the_numbering_report_lists_frozen_tags_then_frozen_wire_numbers() {
    let m = model();
    let q = Q::new(&m);
    let block: ReportBlock = serde_json::from_value(
        json!({"id": uid(0x50), "source": {"kind": "numbering_report"}, "form": uid(0x51)}),
    )
    .expect("a report block");
    let set = Registry::builtin()
        .by_name("numbering_report")
        .expect("a registered kind")
        .rows(&q, &block);
    let rows: Vec<Vec<String>> = (0..set.rows.len()).map(|i| row_texts(&q, &set, i)).collect();
    assert_eq!(rows.len(), 4, "{rows:#?}");
    let tags: Vec<[&str; 4]> = rows[..3]
        .iter()
        .map(|r| [r[1].as_str(), r[2].as_str(), r[3].as_str(), r[4].as_str()])
        .collect();
    assert_eq!(
        tags,
        [
            ["tag", "K1", "K1", "matches"],
            ["tag", "K9", "K3", "deviates"],
            ["tag", "K10", "K2", "deviates"],
        ]
    );
    for (row, frozen) in rows[..3].iter().zip(["K1", "K9", "K10"]) {
        assert!(row[0].ends_with(frozen), "the item is the device's tag: {row:?}");
    }
    let wire = &rows[3];
    assert_eq!(
        [
            wire[0].as_str(),
            wire[1].as_str(),
            wire[2].as_str(),
            wire[4].as_str()
        ],
        ["", "wire number", "W5", "deviates"]
    );
    assert_ne!(wire[3], "W5");
}
