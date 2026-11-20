// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CSV exports of a project without their blocks (FR-REP-005): a project that holds no parts summary or cable
//! overview block has nothing to export. The assembly mode of the parts list and the break marks of an assembly's
//! overview are `ew-derive`'s `bom.rs` and `fixture_tables.rs`.

use ew_api::{BomMode, Export, ExportError, Session, SessionOptions};

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
