// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one front door of every importer call and the importer contract: each new
//! `Limits` member refuses the smallest input above it, naming the limit; the four contract breaches; an
//! importer that panics.

use ew_api::import::{
    ContractBreach, ImportBatch, ImportError, ImportReport, Limits, Refusal, check_contract, front_door,
};
use ew_ids::Uuid;
use ew_import_catalog::ew_interop_api::{ImportInput, SourceInfo};
use ew_import_catalog::{CatalogTarget, ColumnMap, parse_parts};
use ew_schema::Record;

fn map() -> ColumnMap {
    ColumnMap::new(None, [("manufacturer", "Maker"), ("part_number", "Order code")]).unwrap()
}

/// The parts importer through the front door, under `limits`.
fn parts(name: &str, bytes: Vec<u8>, limits: Limits) -> Result<usize, Refusal> {
    let input = ImportInput::new(name, bytes).unwrap();
    let target = CatalogTarget {
        limits,
        ..CatalogTarget::new(Uuid::from_u128(1))
    };
    let sizes = [input.bytes().len() as u64];
    front_door(name, &sizes, &limits, None, || {
        parse_parts(&input, &map(), &[], &target)
    })
    .map(|i| i.rows.len())
}

/// A workbook of `rows` data rows (the program's own XLSX writer).
fn workbook(rows: usize) -> Vec<u8> {
    use ew_api::output::{Table, TableCell, TableColumn, TableRow, WorkbookMeta, xlsx};
    let column = |key: &str| TableColumn {
        key: key.into(),
        label: key.into(),
        cell: ew_reports::CellType::Text,
        editable: false,
    };
    let table = Table {
        name: "Parts".into(),
        key: "sql".into(),
        columns: vec![column("Maker"), column("Order code")],
        rows: (0..rows)
            .map(|i| TableRow {
                id: None,
                heading: false,
                cells: vec![
                    TableCell::Text("Generic".into()),
                    TableCell::Text(format!("P-{i}")),
                ],
            })
            .collect(),
    };
    let meta = WorkbookMeta {
        date: ew_reports::xlsx::DateTime {
            year: 2026,
            month: 11,
            day: 4,
            hour: 0,
            minute: 0,
            second: 0,
        },
        project: Uuid::from_u128(1).to_string(),
        rev: 1,
        generation: None,
        version: "0.1.0".into(),
        base_sha256: None,
    };
    xlsx(&[table], &meta).unwrap()
}

/// The `(declared size, packed size)` of every entry of a ZIP's central directory.
fn entries(zip: &[u8]) -> Vec<(u64, u64)> {
    let u16_at = |at: usize| u64::from(u16::from_le_bytes([zip[at], zip[at + 1]]));
    let u32_at = |at: usize| u64::from(u32::from_le_bytes(zip[at..at + 4].try_into().unwrap()));
    let eocd = (0..zip.len() - 21)
        .rev()
        .find(|&i| zip[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .unwrap();
    let mut at = u32_at(eocd + 16) as usize;
    (0..u16_at(eocd + 10))
        .map(|_| {
            let e = (u32_at(at + 24), u32_at(at + 20));
            at += 46 + (u16_at(at + 28) + u16_at(at + 30) + u16_at(at + 32)) as usize;
            e
        })
        .collect()
}

fn limit_text(what: &str, limit: u64) -> String {
    format!("input exceeds the {what} limit of {limit}")
}

/// IC-1: the smallest workbook above each archive member, and the table members, refuse the import by name.
#[test]
fn each_new_limit_refuses_the_smallest_input_above_it() {
    let book = workbook(3);
    assert_eq!(parts("list.xlsx", book.clone(), Limits::default()), Ok(3));
    let dir = entries(&book);
    let below = |f: &dyn Fn(&mut Limits)| {
        let mut l = Limits::default();
        f(&mut l);
        parts("list.xlsx", book.clone(), l).unwrap_err().to_string()
    };
    let n = dir.len() as u32;
    assert_eq!(
        below(&|l| l.max_archive_entries = n - 1),
        limit_text("archive entry count", u64::from(n - 1))
    );
    assert_eq!(
        parts(
            "list.xlsx",
            book.clone(),
            Limits {
                max_archive_entries: n,
                ..Limits::default()
            }
        ),
        Ok(3)
    );
    let largest = dir.iter().map(|e| e.0).max().unwrap();
    assert_eq!(
        below(&|l| l.max_entry_bytes = largest - 1),
        limit_text("archive entry size", largest - 1)
    );
    let total: u64 = dir.iter().map(|e| e.0).sum();
    assert!(
        (book.len() as u64) < total - 1,
        "the workbook packs smaller than it expands"
    );
    assert_eq!(
        below(&|l| l.max_expanded_bytes = total - 1),
        limit_text("expanded archive size", total - 1)
    );
    let ratio = dir
        .iter()
        .filter(|e| e.1 > 0)
        .map(|e| (e.0 - 1) / e.1)
        .max()
        .unwrap();
    assert!(ratio >= 1, "a deflated entry");
    assert_eq!(
        below(&|l| l.max_entry_ratio = ratio as u32),
        limit_text("archive entry expansion ratio", ratio)
    );
    assert_eq!(below(&|l| l.max_table_rows = 2), limit_text("row count", 2));
    assert_eq!(below(&|l| l.max_table_columns = 1), limit_text("column count", 1));
    let csv = b"Maker,Order code\nGeneric,P-1\nGeneric,P-2\n".to_vec();
    let refused = |l: Limits| parts("list.csv", csv.clone(), l).unwrap_err().to_string();
    assert_eq!(
        refused(Limits {
            max_table_rows: 1,
            ..Limits::default()
        }),
        limit_text("row count", 1)
    );
    assert_eq!(
        refused(Limits {
            max_table_columns: 1,
            ..Limits::default()
        }),
        limit_text("column count", 1)
    );
}

/// IC-1: the front door's own checks come before the importer runs: a set of more files than `max_input_files`, a
/// total above `max_expanded_bytes`, a file above `max_input_bytes`.
#[test]
fn the_front_door_refuses_a_set_before_the_importer_runs() {
    let run = || -> Result<ImportBatch, ImportError> { panic!("the importer must not run") };
    let door = |sizes: &[u64], limits: Limits| {
        front_door("set", sizes, &limits, None, run)
            .unwrap_err()
            .to_string()
    };
    let l = Limits {
        max_input_files: 2,
        max_expanded_bytes: 10,
        max_input_bytes: 6,
        ..Limits::default()
    };
    assert_eq!(door(&[1, 1, 1], l), limit_text("input file count", 2));
    assert_eq!(door(&[7], l), limit_text("input size", 6));
    assert_eq!(door(&[6, 5], l), limit_text("expanded input size", 10));
}

fn batch(records: Vec<Record>) -> ImportBatch {
    ImportBatch {
        records,
        lib: Vec::new(),
        assets: Vec::new(),
        report: ImportReport::new("test", "1", SourceInfo::default()),
    }
}

/// IC-4 (NFR-REL-12): an importer that panics refuses the import with T1, and the session it would have changed is
/// unchanged.
#[test]
fn an_importer_that_panics_is_refused_with_t1() {
    let session = ew_api::Session::from_records(
        serde_json::from_str(
            r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
        )
        .unwrap(),
        [],
        [],
        ew_api::SessionOptions::default(),
    )
    .unwrap();
    let before = session.world().rev;
    let r = front_door(
        "plant.qet",
        &[3],
        &Limits::default(),
        None,
        || -> Result<ImportBatch, ImportError> { panic!("an importer bug") },
    );
    assert_eq!(
        r,
        Err(Refusal::Panicked {
            file: "plant.qet".into()
        })
    );
    assert_eq!(
        r.unwrap_err().to_string(),
        "The importer stopped on an internal error while reading plant.qet; nothing was imported."
    );
    assert_eq!(session.world().rev, before);
}

/// IC-3 (FR-IMP-002): four hand-made batches, one per breach, each refused with T2's reason; their repaired form passes.
#[test]
fn the_contract_refuses_each_breach_with_its_reason() {
    use ew_import_qet::ew_interop_api::LineageInfo;
    let ns = Uuid::from_u128(0x5000);
    let lineage = LineageInfo {
        id: ew_ids::Id::from_uuid(Uuid::from_u128(0x15)),
        namespace: ns,
    };
    let page = |key: &str, lineage: Uuid, id: Option<Uuid>| -> Record {
        let id = id.unwrap_or_else(|| Uuid::new_v5(&ns, key.as_bytes()));
        Record::from_json(&format!(
            r#"{{"$k":"page","id":"{id}","folio":{{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":1}},"kind":"multi_line","grid":{{"num":7937500,"den":3}},"source_ref":{{"lineage":"{lineage}","key":"{key}"}}}}"#
        ))
        .unwrap()
    };
    let l = Uuid::from_u128(0x15);
    let mut good = vec![page("p1", l, None), page("p2", l, None)];
    good.sort_by_key(|r| r.id());
    assert_eq!(check_contract(&batch(good.clone()), Some(&lineage)), Ok(()));
    let cases = [
        (
            vec![good[0].clone(), good[0].clone()],
            ContractBreach::SharedIdentity,
        ),
        (
            vec![page("p1", l, Some(Uuid::from_u128(9)))],
            ContractBreach::IdentityNotFromKey,
        ),
        (
            vec![page("p1", Uuid::from_u128(0x16), None)],
            ContractBreach::OtherLineage,
        ),
        (vec![good[1].clone(), good[0].clone()], ContractBreach::OutOfOrder),
    ];
    let texts = [
        "two records share one identity",
        "a record's identity does not follow from its source key",
        "a record names another import",
        "the records are out of order",
    ];
    for ((records, breach), reason) in cases.into_iter().zip(texts) {
        let b = batch(records);
        assert_eq!(check_contract(&b, Some(&lineage)), Err(breach));
        let refused =
            front_door("plant.qet", &[1], &Limits::default(), Some(&lineage), || Ok(b)).unwrap_err();
        assert_eq!(
            refused.to_string(),
            format!("The import of plant.qet was refused because {reason}; nothing was imported.")
        );
    }
}
