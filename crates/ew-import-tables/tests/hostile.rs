// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hostile input (doc 14 §11): file, size, row, column and cell limits, the report cap and arbitrary bytes.

mod common;

use ew_import_tables::csv::TableLimits;
use ew_import_tables::{MAX_REPORT_ENTRIES, MAX_TABLE_FILES, TablesImporter};
use ew_interop_api::{Code, ImportError};

/// More than `MAX_TABLE_FILES` inputs are refused.
#[test]
fn too_many_files_are_refused() {
    let names: Vec<String> = (0..=MAX_TABLE_FILES).map(|i| format!("t{i}.csv")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "key\n")).collect();
    let r = common::import(&common::inputs(&files));
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

/// A total above `ctx.limits.max_input_bytes` is refused.
#[test]
fn an_oversized_total_is_refused() {
    let mut ctx = common::context();
    ctx.limits.max_input_bytes = 16;
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV)]);
    let r = TablesImporter::new(common::library()).import(&inputs, &ctx);
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

fn limited(max_rows: usize, max_columns: usize, max_cell_bytes: usize) -> TablesImporter {
    TablesImporter::new(common::library()).with_limits(TableLimits {
        max_rows,
        max_columns,
        max_cell_bytes,
        ..TableLimits::default()
    })
}

fn run_limited(importer: &TablesImporter, devices: &str) -> Result<(), ImportError> {
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV), ("devices.csv", devices)]);
    importer.import(&inputs, &common::context()).map(|_| ())
}

/// A table with more rows than `max_rows` is refused.
#[test]
fn too_many_rows_are_refused() {
    let importer = limited(3, 1_024, 32 * 1024);
    assert!(run_limited(&importer, "key,letter\na,F\nb,F\nc,F\n").is_ok());
    let r = run_limited(&importer, "key,letter\na,F\nb,F\nc,F\nd,F\n");
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

/// A table with more columns than `max_columns` is refused.
#[test]
fn too_many_columns_are_refused() {
    let importer = limited(3, 3, 32 * 1024);
    assert!(run_limited(&importer, "key,letter,counter\na,F,1\n").is_ok());
    let r = run_limited(&importer, "key,letter,counter,parent\na,F,1,\n");
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

/// A cell longer than `max_cell_bytes` is refused.
#[test]
fn a_long_cell_is_refused() {
    // 20 bytes fit (the longest header cell of project.csv is 18); 21 do not.
    let importer = limited(3, 8, 20);
    assert!(run_limited(&importer, "key,letter\naaaaaaaaaaaaaaaaaaaa,F\n").is_ok());
    let r = run_limited(&importer, "key,letter\naaaaaaaaaaaaaaaaaaaaa,F\n");
    assert!(matches!(r, Err(ImportError::LimitExceeded { .. })), "{r:?}");
}

/// 1,500 failing rows list `MAX_REPORT_ENTRIES` findings and one EW-IMP-001 summary.
#[test]
fn the_report_is_capped() {
    let mut devices = String::from("key,letter\n");
    for i in 0..1_500 {
        devices.push_str(&format!("d{i:04},\n"));
    }
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV), ("devices.csv", &devices)]);
    let b = common::import(&inputs).expect("the import succeeds");
    assert_eq!(b.report.entries.len(), MAX_REPORT_ENTRIES + 1);
    let last = b.report.entries.last().unwrap();
    assert_eq!(last.code, Code::imp(1));
    assert!(last.message.contains("500"), "{}", last.message);
    assert_eq!(b.report.with_code(Code::imp(1)).count(), 1);
}

/// A deterministic LCG byte generator (no dependency on a random crate).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn bytes(&mut self) -> Vec<u8> {
        const SPECIAL: &[u8] = b",\"\n;:/@#";
        const PLAIN: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_. ";
        let len = (self.next() % 2_001) as usize;
        (0..len)
            .map(|_| match self.next() % 10 {
                0..=3 => SPECIAL[(self.next() as usize) % SPECIAL.len()],
                4..=8 => PLAIN[(self.next() as usize) % PLAIN.len()],
                _ => (self.next() & 0xFF) as u8,
            })
            .collect()
    }
}

/// Random bytes in place of one golden table never panic the importer.
#[test]
fn arbitrary_bytes_never_panic() {
    let golden = common::golden_inputs();
    let importer = TablesImporter::new(common::library());
    let ctx = common::context();
    let mut rng = Lcg(0x5EED_1234_ABCD_0042);
    let junk: Vec<Vec<u8>> = (0..300).map(|_| rng.bytes()).collect();
    // One thread per replaced table keeps the debug-build run short.
    std::thread::scope(|scope| {
        for target in [
            "devices.csv",
            "connections.csv",
            "functions.csv",
            "representations.csv",
        ] {
            let (golden, importer, ctx, junk) = (&golden, &importer, &ctx, &junk);
            scope.spawn(move || {
                for bytes in junk {
                    let mut inputs: Vec<_> = golden.iter().filter(|i| i.name() != target).cloned().collect();
                    inputs.push(ew_interop_api::ImportInput::new(target, bytes.clone()).unwrap());
                    let _ = importer.import(&inputs, ctx);
                }
            });
        }
    });
}
