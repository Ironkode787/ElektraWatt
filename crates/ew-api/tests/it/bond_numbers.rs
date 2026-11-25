// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The potential overview numbers its bonds the declared ones first, then the drawn ones (doc 09 §2.4): an
//! earth wire drawn before a bond was declared does not take its number.

use crate::common;

use ew_ids::Uuid;
use ew_reports::render::{RenderOptions, render};
use ew_schema::Record;
use serde_json::{Value, json};

/// The function definition of the sample's power supply (`ew.fn.psu-ac-dc`): CP 2 is its PE pin, CP 6 a 0 V pin.
const PSU_DEF: &str = "a5722bff-01d3-5a05-b24f-ec54f7252a45";
const PE: u16 = 2;
const ZERO_VOLT: u16 = 6;

#[test]
fn a_drawn_bond_never_takes_a_declared_bonds_number() {
    let (project, records, lib) = common::sample_records();
    let mut values: Vec<Value> = records
        .iter()
        .map(|r| serde_json::to_value(r).expect("a record"))
        .collect();
    // The sample's declared bond, and its power supply's function: a 0 V pin and a PE pin.
    let bond: Uuid = values
        .iter()
        .find(|v| v["$k"] == "bond")
        .and_then(|v| v["id"].as_str()?.parse().ok())
        .expect("the sample's bond");
    let supply = values
        .iter()
        .find(|v| v["$k"] == "function" && v["kind"] == "black_box" && v["def"] == PSU_DEF)
        .expect("the sample's power supply")
        .clone();
    let page = values
        .iter()
        .find(|v| v["$k"] == "connection" && v["a"]["cp"][0] == supply["id"])
        .map(|v| v["page"].clone())
        .expect("its page");
    // An earth wire from its 0 V pin to its PE pin, drawn before the bond was declared (an ID before the bond's).
    let wire = Uuid::from_u128(bond.as_u128() - 1);
    values.push(json!({
        "$k": "connection",
        "id": wire.to_string(),
        "a": {"cp": [supply["id"], ZERO_VOLT]},
        "b": {"cp": [supply["id"], PE]},
        "page": page,
    }));
    let records: Vec<Record> = values
        .into_iter()
        .map(|v| Record::from_value(v).expect("a record"))
        .collect();
    let s = common::session_of(project, records, lib);
    let q = s.query();
    let rows =
        ew_reports::project_rows(&q, &ew_schema::ReportKind::PotentialOverview, None).expect("the kind");
    let opts = RenderOptions::default();
    let cell = |row: &ew_reports::Row, key: &str| {
        rows.column(key)
            .and_then(|i| row.cells.get(i))
            .map(|c| render(&q, c, &opts))
            .unwrap_or_default()
    };
    // Each bond row: its number and its record (the bond's, or a drawn bond's wire).
    let bonds: Vec<(String, Option<Uuid>)> = rows
        .rows
        .iter()
        .filter(|r| cell(r, "kind") == "Bond")
        .map(|r| {
            let label = cell(r, "potential");
            let number = label.split(' ').next().unwrap_or_default().to_owned();
            let record = match r.id.0.first() {
                Some(ew_reports::RowKey::Entity(id)) => Some(*id),
                _ => None,
            };
            (number, record)
        })
        .collect();
    let number = |id: Uuid| {
        bonds
            .iter()
            .find(|(_, r)| *r == Some(id))
            .map(|(n, _)| n.as_str())
    };
    assert_eq!(number(bond), Some("B1"), "the declared bond: {bonds:?}");
    assert!(
        number(wire).is_some_and(|n| n != "B1"),
        "the earth wire after it: {bonds:?}"
    );
}
