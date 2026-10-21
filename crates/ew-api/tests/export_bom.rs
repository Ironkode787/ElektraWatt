// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The BOM export over a model with configured products (doc 20 §14.2, FR-REP-005, D-36).
//!
//! The golden tables hold no configured product yet, so the assembly rows A1 and A2 are tested over the golden
//! model with two configured parts and the assembly aspect of doc 20 §14.2 added to -201U1 (order code 1 covers
//! the head, order code 2 its descendants): the export replaces rows 16-21 by two `set` rows and leaves every
//! other row of the parts list as it was.

mod golden_common;

use ew_api::{BomMode, Export, ExportError, Session, SessionOptions};
use ew_schema::{LibDocument, Record};
use serde_json::json;

const CPX: &str = "51E-F13GCQP-D+H2GG";
const MPA: &str = "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS";
const CPX_COPY: &str = "5e2f0c1a-7b3d-5c11-8a42-000000000001";
const MPA_COPY: &str = "5e2f0c1a-7b3d-5c11-8a42-000000000002";

/// The golden model with the configured parts of §14.2 named by -201U1's assembly aspect (CPX first, MPA
/// second).
fn configured() -> Session {
    let batch = golden_common::batch();
    let mut lib = batch.lib.clone();
    let node = lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(p) if p.item.part_number.as_str() == "195740" => Some(d.clone()),
            _ => None,
        })
        .expect("the CPX-FB13 bus node is in the golden library");
    for (n, (copy, code, text)) in [
        (CPX_COPY, CPX, "CPX terminal (order code 1)"),
        (MPA_COPY, MPA, "MPA valve terminal (order code 2)"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut v = serde_json::to_value(&node).expect("a part serialises");
        v["id"] = json!(copy);
        v["item_id"] = json!(format!("5e2f0c1a-7b3d-5c11-8a42-0000000001{n:02}"));
        v["part_number"] = json!(code);
        v["description"] = json!(text);
        v["kind"] = json!("configured");
        v["unit"] = json!("set");
        v["meta"] = json!({"key": code});
        lib.push(LibDocument::from_json(&v.to_string()).expect("a configured part"));
    }
    // -201U1 and its descendants (by `parent`).
    let head = batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if golden_common::source_key(r) == Some("devices|-201U1") => Some(d.id),
            _ => None,
        })
        .expect("-201U1 is in the batch");
    let mut descendants: Vec<String> = Vec::new();
    let mut frontier = vec![head];
    while let Some(parent) = frontier.pop() {
        for r in &batch.records {
            if let Record::Device(d) = r
                && d.parent == Some(parent)
            {
                descendants.push(d.id.to_string());
                frontier.push(d.id);
            }
        }
    }
    descendants.sort();
    let assembly = json!({"configs": [
        {"order_code": CPX, "part": {"copy": CPX_COPY}, "covers": [head.to_string()]},
        {"order_code": MPA, "part": {"copy": MPA_COPY}, "covers": descendants},
    ]});
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            Record::Device(d) if d.id == head => {
                let mut v = serde_json::to_value(d).expect("a device serialises");
                v["assembly"] = assembly.clone();
                records.push(Record::Device(serde_json::from_value(v).expect("a device")));
            }
            other => records.push(other.clone()),
        }
    }
    Session::from_records(
        project.expect("the batch holds the project"),
        records,
        lib,
        SessionOptions::default(),
    )
    .expect("the golden model with configured parts passes the integrity check")
}

fn rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

#[test]
fn assembly_mode_replaces_rows_16_to_21_by_the_two_configured_products() {
    let s = configured();
    let component = s
        .export_csv(Export::Bom(BomMode::Component))
        .expect("the parts list");
    let assembly = s
        .export_csv(Export::Bom(BomMode::Assembly))
        .expect("the assembly list");
    // The configured parts are no component part: the parts list is the golden one (27 rows).
    assert_eq!(component.lines().count(), 28);
    assert!(!component.contains(CPX));
    // §14.2: rows 1-15, A1, A2, rows 22-27.
    let lines: Vec<&str> = assembly.lines().collect();
    assert_eq!(lines.len(), 1 + 15 + 2 + 6, "{assembly}");
    let comp: Vec<&str> = component.lines().collect();
    assert_eq!(lines[1..16], comp[1..16], "rows 1-15 stay");
    // Rows 22-27 stay, renumbered 16-21: the positions run over the component rows that are left.
    let without_pos = |l: &str| l.splitn(3, ',').collect::<Vec<_>>()[2].to_owned();
    let tail: Vec<String> = lines[18..].iter().map(|l| without_pos(l)).collect();
    let kept: Vec<String> = comp[22..].iter().map(|l| without_pos(l)).collect();
    assert_eq!(tail, kept, "rows 22-27 stay");
    assert!(lines[18].starts_with("data,16,"), "{}", lines[18]);
    let a = rows(&assembly);
    for (row, (pos, code, text)) in a[15..17].iter().zip([
        ("A1", CPX, "CPX terminal (order code 1)"),
        ("A2", MPA, "MPA valve terminal (order code 2)"),
    ]) {
        // row, Pos, Qty, Unit, Manufacturer, Part number (the order code), Description, Tags, Folios
        assert_eq!(row[1], pos);
        assert_eq!(row[2..6], ["1", "set", "Festo", code]);
        assert!(row[6].starts_with(text), "{row:?}");
        assert_eq!(row[7], "-201U1");
    }
}

#[test]
fn a_project_without_the_block_cannot_export() {
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .expect("a project row");
    let s = Session::from_records(project, [], [], SessionOptions::default()).expect("an empty model");
    assert_eq!(
        s.export_csv(Export::Cables),
        Err(ExportError::NoBlock("cable_overview_diagrams"))
    );
    assert_eq!(
        s.export_csv(Export::Bom(BomMode::Component)),
        Err(ExportError::NoBlock("parts_summary"))
    );
}
