// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The BOM export and the assembly of the golden project (doc 20 §14.2, §19 D-36 and D-41; FR-REP-005,
//! FR-DEV-020 … FR-DEV-023): -201U1 carries the typed assembly aspect with order codes 1 and 2, so the
//! assembly export replaces rows 16-21 by A1 and A2, and its overview over folios 201 and 202 gets break marks.

mod golden_common;

use std::collections::BTreeMap;

use ew_api::{BomMode, Export, ExportError, Session, SessionOptions};
use ew_derive::{AssemblyBreakMarks, BreakSide};
use ew_ids::Id;
use ew_schema::{Device, Representation};

const CPX: &str = "51E-F13GCQP-D+H2GG";
const MPA: &str = "32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS";

fn rows(csv: &str) -> Vec<Vec<String>> {
    csv.lines()
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}

fn golden() -> Session {
    golden_common::session(&golden_common::batch())
}

/// The record IDs of the golden batch by source key (`devices|-201U1`).
fn ids() -> BTreeMap<String, String> {
    golden_common::ids_by_key(&golden_common::batch())
}

fn device(ids: &BTreeMap<String, String>, tag: &str) -> Id<Device> {
    ids[&format!("devices|{tag}")].parse().expect("a device ID")
}

fn rep(ids: &BTreeMap<String, String>, key: &str) -> Id<Representation> {
    ids[&format!("representations|{key}")]
        .parse()
        .expect("a representation ID")
}

#[test]
fn assembly_mode_replaces_rows_16_to_21_by_the_two_configured_products() {
    let s = golden();
    let component = s
        .export_csv(Export::Bom(BomMode::Component))
        .expect("the parts list");
    let assembly = s
        .export_csv(Export::Bom(BomMode::Assembly))
        .expect("the assembly list");
    // The configured products are no component part: the parts list is the golden one (27 rows).
    assert_eq!(component.lines().count(), 28);
    assert!(!component.contains(CPX));
    // §14.2: rows 1-15, A1, A2, rows 22-27; one "Part number" column in both modes, "Covers" appended.
    let lines: Vec<&str> = assembly.lines().collect();
    assert_eq!(lines.len(), 1 + 15 + 2 + 6, "{assembly}");
    assert_eq!(
        lines[0],
        "row,Pos,Qty,Unit,Manufacturer,Part number,Description,Tags,Folios,Covers"
    );
    let comp: Vec<&str> = component.lines().collect();
    for (a, c) in lines[1..16].iter().zip(&comp[1..16]) {
        assert_eq!(*a, format!("{c},"), "rows 1-15 stay, with an empty Covers cell");
    }
    // Rows 22-27 stay, renumbered 16-21: the positions run over the component rows that are left (ID-356).
    let without_pos = |l: &str| l.splitn(3, ',').collect::<Vec<_>>()[2].to_owned();
    let tail: Vec<String> = lines[18..].iter().map(|l| without_pos(l)).collect();
    let kept: Vec<String> = comp[22..]
        .iter()
        .map(|l| format!("{},", without_pos(l)))
        .collect();
    assert_eq!(tail, kept, "rows 22-27 stay");
    assert!(lines[18].starts_with("data,16,"), "{}", lines[18]);
    let a = rows(&assembly);
    for (row, (pos, code, text)) in a[15..17].iter().zip([
        ("A1", CPX, "CPX terminal (order code 1)"),
        ("A2", MPA, "MPA valve terminal (order code 2)"),
    ]) {
        // row, Pos, Qty, Unit, Manufacturer, Part number (the order code), Description, Tags, Folios, Covers
        assert_eq!(row[1], pos);
        assert_eq!(row[2..6], ["1", "set", "Festo", code]);
        assert_eq!(row[6], text);
        assert_eq!(row[7], "-201U1");
        assert_eq!(row[8], "201");
    }
    assert!(
        lines[16].ends_with(",-201U1"),
        "A1 covers the head: {}",
        lines[16]
    );
    assert!(
        lines[17].contains("-201U2"),
        "A2 covers the children: {}",
        lines[17]
    );
}

#[test]
fn the_overview_of_the_assembly_breaks_between_201_and_202() {
    let s = golden();
    let ids = ids();
    let marks = s.query().get::<AssemblyBreakMarks>(()).as_ref().clone();
    let got: Vec<(Id<Representation>, BreakSide, Id<Device>)> =
        marks.iter().map(|m| (m.rep, m.side, m.assembly)).collect();
    let head = device(&ids, "-201U1");
    assert_eq!(
        got,
        {
            let mut want = vec![
                (rep(&ids, "-201U9"), BreakSide::Right, head),
                (rep(&ids, "-202U1"), BreakSide::Left, head),
            ];
            want.sort_by_key(|(r, side, _)| (marks.iter().find(|m| m.rep == *r).map(|m| m.page), *r, *side));
            want
        },
        "right after -201U9 on 201, left before -202U1 on 202"
    );
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
