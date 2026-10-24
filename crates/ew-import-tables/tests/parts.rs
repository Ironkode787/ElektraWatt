// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410: `parse_parts_tables`, the strict parts import into an existing project. The golden tables give the
//! bootstrap's part copies (but for the provenance), deterministically; a missing or unknown column and a bad row
//! refuse the whole import with their pointer.

mod common;

use common::{golden_inputs, library, tables_dir};
use ew_ids::{Id, Uuid};
use ew_import_tables::{SiblingTables, TablesImporter, bootstrap_context, ids, parse_parts_tables};
use ew_interop_api::ImportError;
use ew_schema::{LibCopy, LibDocument, Part, Project};

fn read(name: &str) -> String {
    std::fs::read_to_string(tables_dir().join(name)).unwrap()
}

/// The golden parts tables, all five.
fn golden() -> (String, Vec<(&'static str, String)>) {
    let siblings = SiblingTables::FILE_NAMES.iter().map(|n| (*n, read(n))).collect();
    (read("parts.csv"), siblings)
}

fn parse(
    parts: &str,
    siblings: &[(&'static str, String)],
    project: Id<Project>,
) -> Result<Vec<LibCopy<Part>>, ImportError> {
    let mut s = SiblingTables::default();
    for (n, t) in siblings {
        assert!(s.set(n, t));
    }
    parse_parts_tables(parts, &s, project, "some/dir/parts.csv", &library())
}

fn project() -> Id<Project> {
    Id::from_uuid(Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0001))
}

#[test]
fn the_golden_tables_give_the_bootstraps_part_copies() {
    let (parts, siblings) = golden();
    let golden_project = ids::project_id("J-E-2416");
    let copies = parse(&parts, &siblings, golden_project).expect("the golden parts tables import");
    let batch = TablesImporter::new(library())
        .import(&golden_inputs(), &bootstrap_context())
        .unwrap();
    let boot: Vec<&LibCopy<Part>> = batch
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Part(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 29);
    assert_eq!(copies.len(), boot.len());
    for c in &copies {
        let b = boot
            .iter()
            .find(|b| b.id == c.id)
            .expect("the same copy identity");
        assert_eq!(
            (c.library_id, c.item_id, &c.version),
            (b.library_id, b.item_id, &b.version)
        );
        assert_eq!(
            c.item.meta.sources,
            vec!["parts.csv".to_owned()],
            "the base name only"
        );
        let mut item = c.item.clone();
        item.meta.sources.clone_from(&b.item.meta.sources);
        assert_eq!(item, b.item, "{}", c.item.part_number);
    }
}

#[test]
fn copies_are_project_local_and_deterministic() {
    let (parts, siblings) = golden();
    let a = parse(&parts, &siblings, project()).unwrap();
    let b = parse(&parts, &siblings, project()).unwrap();
    assert_eq!(a, b);
    for c in &a {
        assert_eq!(c.library_id.as_uuid(), project().as_uuid());
        let key = c.item.meta.key.as_ref().unwrap().to_string();
        assert_eq!(c.item_id, ids::record_id("parts", &key));
        assert_eq!(
            c.id,
            ew_ids::v5::lib_copy_id(c.library_id.as_uuid(), c.item_id.as_uuid())
        );
    }
}

#[test]
fn without_siblings_the_import_is_parts_only() {
    let (parts, _) = golden();
    let copies = parse(&parts, &[], project()).unwrap();
    assert_eq!(copies.len(), 29);
    assert!(
        copies
            .iter()
            .all(|c| c.item.cp_pattern.is_empty() && c.item.function_template.is_empty())
    );
}

#[test]
fn a_missing_column_is_refused_naming_it() {
    let e = parse("key,part_number\nA,A1\n", &[], project()).unwrap_err();
    let ImportError::Malformed { message, line } = &e else {
        panic!("{e}")
    };
    assert_eq!(*line, Some(1));
    assert!(
        message.contains("parts.csv") && message.contains("manufacturer"),
        "{message}"
    );
}

#[test]
fn an_unknown_column_is_refused_naming_it() {
    let e = parse(
        "key,manufacturer,part_number,colour\nA,Acme,A1,red\n",
        &[],
        project(),
    )
    .unwrap_err();
    let ImportError::Malformed { message, line } = &e else {
        panic!("{e}")
    };
    assert_eq!(*line, Some(1));
    assert!(message.contains("unknown column `colour`"), "{message}");
}

#[test]
fn a_bad_row_refuses_the_whole_import_with_its_pointer() {
    let text = "key,manufacturer,part_number,kind\nA,Acme,A1,device\nB,Acme,B1,gizmo\n";
    let e = parse(text, &[], project()).unwrap_err();
    assert_eq!(
        e.to_string(),
        "malformed input at line 3: parts.csv, row `B`: column `kind`: `gizmo` is not one of device, terminal, \
         cable, wire, ferrule, marker, duct, rail, busbar, end_bracket, enclosure, mounting_plate, tube, accessory, \
         configured"
            .replace("\\\n", ""),
        "{e}"
    );
}

#[test]
fn a_key_that_is_not_an_identifier_is_refused_with_its_pointer() {
    // A control character inside the key (a tab): `Ident` refuses it, so the copy's key cannot be set.
    let text = "key,manufacturer,part_number\nA,Acme,A1\nB\tC,Acme,B1\n";
    let e = parse(text, &[], project()).unwrap_err();
    let ImportError::Malformed { message, line } = &e else {
        panic!("{e}")
    };
    assert_eq!(*line, Some(3));
    assert!(
        message.starts_with("parts.csv, row `B\tC`: column `key`:"),
        "{message}"
    );
}

#[test]
fn a_child_row_of_an_unknown_part_is_refused() {
    let cps = "key,part,idx,designation\nX:0,X,0,1\n".to_owned();
    let e = parse(
        "key,manufacturer,part_number\nA,Acme,A1\n",
        &[("part_cps.csv", cps)],
        project(),
    )
    .unwrap_err();
    let ImportError::Malformed { message, line } = &e else {
        panic!("{e}")
    };
    assert_eq!(*line, Some(2));
    assert!(
        message.starts_with("part_cps.csv, row `X:0`: unknown part `X`"),
        "{message}"
    );
}
