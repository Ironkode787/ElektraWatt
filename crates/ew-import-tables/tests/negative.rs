// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Defective tables on small in-memory inputs: what is refused, what is dropped and what is kept with a finding.

mod common;

use ew_ids::Uuid;
use ew_import_tables::TablesImporter;
use ew_import_tables::ids;
use ew_interop_api::{Code, ImportBatch, ImportError, LineageInfo, Severity};
use ew_schema::{Device, Page, Record, Representation};
use ew_units::Grid;

const PAGES: &str = "key,number,kind,form,grid\n101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n";
const DEVICES: &str = "key,letter,counter\n-1F1,F,1\n";
const FUNCTIONS: &str = "key,device,fndef\n-1F1,-1F1,ew.fn.mcb-1pn\n";
const REPS: &str = "key,functions,page,symbol,cell,dx,dy\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,,\n";

/// The base tables with the files of `replace` substituted by name and the rest of `extra` appended.
fn run_with(replace: &[(&str, &str)]) -> Result<ImportBatch, ImportError> {
    let mut files: Vec<(&str, &str)> = vec![
        ("project.csv", common::PROJECT_CSV),
        ("pages.csv", PAGES),
        ("devices.csv", DEVICES),
        ("functions.csv", FUNCTIONS),
        ("representations.csv", REPS),
    ];
    for (name, text) in replace {
        match files.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 = text,
            None => files.push((name, text)),
        }
    }
    common::import(&common::inputs(&files))
}

fn run(replace: &[(&str, &str)]) -> ImportBatch {
    run_with(replace).expect("the import succeeds")
}

fn count(b: &ImportBatch, code: u16, severity: Severity) -> usize {
    b.report
        .with_code(Code::imp(code))
        .filter(|e| e.severity == severity)
        .count()
}

fn devices(b: &ImportBatch) -> usize {
    b.records
        .iter()
        .filter(|r| matches!(r, Record::Device(_)))
        .count()
}

fn representations(b: &ImportBatch) -> Vec<&Representation> {
    b.records
        .iter()
        .filter_map(|r| match r {
            Record::Representation(x) => Some(x),
            _ => None,
        })
        .collect()
}

/// The base tables import without a single finding.
#[test]
fn the_base_tables_are_clean() {
    let b = run(&[]);
    assert!(b.report.entries.is_empty(), "{:#?}", b.report.entries);
    assert_eq!(devices(&b), 1);
    assert_eq!(representations(&b).len(), 1);
}

/// A table without a required column is refused, and rows that name its keys do not resolve.
#[test]
fn a_missing_required_column_refuses_the_table() {
    let b = run(&[("devices.csv", "key,counter\n-1F1,1\n")]);
    assert_eq!(
        b.report.with_code(Code::imp(2)).count(),
        1,
        "{:#?}",
        b.report.entries
    );
    let e = b.report.with_code(Code::imp(2)).next().unwrap();
    assert_eq!(e.severity, Severity::Error);
    assert_eq!(e.location.as_ref().unwrap().file, "devices.csv");
    assert_eq!(devices(&b), 0);
    assert!(
        count(&b, 3, Severity::Error) >= 1,
        "the function names a device that does not exist"
    );
}

/// A representation on an unknown page is dropped as an unresolved reference.
#[test]
fn a_representation_on_an_unknown_page_is_dropped() {
    let reps = "key,functions,page,symbol,cell\n-1F1,-1F1,999,ew.iec.mcb-1pn@qet10,B2\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 3, Severity::Error), 1, "{:#?}", b.report.entries);
    assert!(representations(&b).is_empty());
}

/// A repeated key in devices.csv drops the second row.
#[test]
fn a_duplicate_key_is_dropped() {
    let b = run(&[("devices.csv", "key,letter,counter\n-1F1,F,1\n-1F1,F,2\n")]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    assert_eq!(devices(&b), 1);
}

/// A position off the page grid is snapped with a warning and the representation is kept.
#[test]
fn an_off_grid_position_is_snapped_with_a_warning() {
    let reps = "key,functions,page,symbol,cell,dx,dy\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,B2,5,\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Warning), 1, "{:#?}", b.report.entries);
    let reps = representations(&b);
    assert_eq!(reps.len(), 1);
    let g = Grid::QET_10PX;
    assert_eq!(
        g.snap_point(reps[0].pos.x),
        Some(reps[0].pos.x),
        "x is on the grid"
    );
    assert_eq!(
        g.snap_point(reps[0].pos.y),
        Some(reps[0].pos.y),
        "y is on the grid"
    );
}

/// A bad cell reference drops the representation.
#[test]
fn a_bad_cell_drops_the_row() {
    let reps = "key,functions,page,symbol,cell\n-1F1,-1F1,101,ew.iec.mcb-1pn@qet10,Z9\n";
    let b = run(&[("representations.csv", reps)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    assert!(representations(&b).is_empty());
}

/// An unknown enumeration value drops the page row; the valid page stays.
#[test]
fn a_bad_enum_value_drops_the_row() {
    let pages = "key,number,kind,form,grid\n\
        101,101,multi_line,ew.frame.qet-a4-landscape-grid,qet10\n\
        102,102,nonsense,ew.frame.qet-a4-landscape-grid,qet10\n";
    let b = run(&[("pages.csv", pages)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    let n = b.records.iter().filter(|r| matches!(r, Record::Page(_))).count();
    assert_eq!(n, 1);
    let gone: ew_ids::Id<Page> = ids::record_id("pages", "102");
    assert!(
        !b.records
            .iter()
            .any(|r| matches!(r, Record::Page(p) if p.id == gone))
    );
}

/// An unknown table file is a warning, a non-CSV file an info; both are ignored.
#[test]
fn unknown_and_non_csv_files_are_reported_and_ignored() {
    let b = run(&[("mystery.csv", "key\nx\n"), ("notes.txt", "hello")]);
    let warn: Vec<_> = b
        .report
        .with_code(Code::imp(4))
        .filter(|e| e.severity == Severity::Warning)
        .collect();
    let info: Vec<_> = b
        .report
        .with_code(Code::imp(4))
        .filter(|e| e.severity == Severity::Info)
        .collect();
    assert_eq!(warn.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(warn[0].location.as_ref().unwrap().file, "mystery.csv");
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].location.as_ref().unwrap().file, "notes.txt");
    assert_eq!(devices(&b), 1);
}

/// A stray quote refuses the table with an error, but the import still succeeds.
#[test]
fn a_malformed_csv_refuses_the_table_only() {
    let b = run(&[("devices.csv", "key,letter\n-1F1,F\"x\n")]);
    let e: Vec<_> = b.report.with_code(Code::imp(2)).collect();
    assert_eq!(e.len(), 1, "{:#?}", b.report.entries);
    assert_eq!(e[0].severity, Severity::Error);
    assert_eq!(e[0].location.as_ref().unwrap().file, "devices.csv");
    assert!(e[0].message.contains("table refused"), "{}", e[0].message);
    assert_eq!(devices(&b), 0);
}

/// Without project.csv the import is refused.
#[test]
fn a_missing_project_table_is_refused() {
    let inputs = common::inputs(&[("pages.csv", PAGES)]);
    let r = common::import(&inputs);
    assert!(matches!(r, Err(ImportError::Malformed { .. })), "{r:?}");
}

/// A foreign lineage is unsupported.
#[test]
fn a_foreign_lineage_is_unsupported() {
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV)]);
    let mut ctx = common::context();
    ctx.lineage = Some(LineageInfo {
        id: ids::lineage_id(),
        namespace: Uuid::from_u128(0x1234_5678_9abc_def0),
    });
    let r = TablesImporter::new(common::library()).import(&inputs, &ctx);
    assert!(matches!(r, Err(ImportError::Unsupported { .. })), "{r:?}");
}

/// A terminal whose key is not `<strip>:<designation>` is dropped.
#[test]
fn a_terminal_key_must_name_strip_and_designation() {
    let devices_csv = "key,letter,counter,strip_label_mode\n-1F1,F,1,\n-1X1,X,1,designation\n";
    let terminals = "key,strip,seq,designation\nbogus,-1X1,1,1\n";
    let b = run(&[("devices.csv", devices_csv), ("terminals.csv", terminals)]);
    assert_eq!(count(&b, 2, Severity::Error), 1, "{:#?}", b.report.entries);
    let n = b
        .records
        .iter()
        .filter(|r| matches!(r, Record::Device(d) if d.aspects.terminal.is_some()))
        .count();
    assert_eq!(n, 0);
    let _: Option<Device> = None;
}
