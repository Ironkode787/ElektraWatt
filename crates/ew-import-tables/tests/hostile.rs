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

/// A table at a row, column or cell-length limit imports and one above it is refused with `LimitExceeded`, one
/// case per limit. For the cell, 20 bytes fit (the longest header cell of project.csv is 18); 21 do not.
#[test]
fn table_limits_are_refused_above_and_kept_at() {
    let cases = [
        (
            "too_many_rows_are_refused",
            limited(3, 1_024, 32 * 1024),
            "key,letter
a,F
b,F
c,F
",
            "key,letter
a,F
b,F
c,F
d,F
",
        ),
        (
            "too_many_columns_are_refused",
            limited(3, 3, 32 * 1024),
            "key,letter,counter
a,F,1
",
            "key,letter,counter,parent
a,F,1,
",
        ),
        (
            "a_long_cell_is_refused",
            limited(3, 8, 20),
            "key,letter
aaaaaaaaaaaaaaaaaaaa,F
",
            "key,letter
aaaaaaaaaaaaaaaaaaaaa,F
",
        ),
    ];
    for (name, importer, at, above) in cases {
        let r = run_limited(&importer, at);
        assert!(r.is_ok(), "case {name}: {r:?}");
        let r = run_limited(&importer, above);
        assert!(
            matches!(r, Err(ImportError::LimitExceeded { .. })),
            "case {name}: {r:?}"
        );
    }
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

/// Values that cell mutations draw from: empty, junk, numeric edges, endpoint and enumeration lookalikes.
const POOL: &[&str] = &[
    "",
    "x",
    "-1",
    "0",
    "65536",
    "99999999999999999999",
    "1.2345",
    "@J1",
    "@nope",
    "ip:nope",
    "-X1031:1/b",
    "-X1031:99/z",
    "A0",
    "I99",
    "H16",
    "a;b;;c",
    "true",
    "maybe",
    "\u{2212}",
    "e\u{301}",
];

/// One cell-wise mutation of `lines`: replace a field (from `POOL`, the key of another row or the value of
/// another cell of the same column), delete a line, duplicate a line or swap two fields. Lines with a quote
/// are left alone by the field mutations (a cell-wise split would break them).
fn mutate_once(lines: &mut Vec<String>, rng: &mut Lcg) {
    if lines.is_empty() {
        return;
    }
    // Data lines are the target; the header is hit only when it is the sole line.
    let target = if lines.len() > 1 {
        1 + (rng.next() as usize) % (lines.len() - 1)
    } else {
        0
    };
    match rng.next() % 10 {
        0..=5 => {
            if lines[target].contains('"') {
                return;
            }
            let mut fields: Vec<String> = lines[target].split(',').map(str::to_owned).collect();
            let col = (rng.next() as usize) % fields.len();
            let other = (rng.next() as usize) % lines.len();
            let value = match rng.next() as usize % (POOL.len() + 2) {
                n if n < POOL.len() => POOL[n].to_owned(),
                n if n == POOL.len() => lines[other].split(',').next().unwrap_or("").to_owned(),
                _ => lines[other].split(',').nth(col).unwrap_or("").to_owned(),
            };
            fields[col] = value;
            lines[target] = fields.join(",");
        }
        6 => {
            lines.remove(target);
        }
        7 => {
            let copy = lines[target].clone();
            lines.insert(target, copy);
        }
        _ => {
            if lines[target].contains('"') {
                return;
            }
            let mut fields: Vec<String> = lines[target].split(',').map(str::to_owned).collect();
            let (a, b) = (
                (rng.next() as usize) % fields.len(),
                (rng.next() as usize) % fields.len(),
            );
            fields.swap(a, b);
            lines[target] = fields.join(",");
        }
    }
}

/// Mutated golden tables never panic: 1 to 3 cell-wise mutations of one table at a time, for every golden
/// table, and every successful batch has unique record IDs.
///
/// Per table: `MUTATIONS_PER_TABLE` = 60 mutated runs from the seed `0xC5F0_7A11_0B5E_ED01` mixed with the
/// table index, one thread per table (debug build: see the measured time in the test run).
#[test]
fn mutated_golden_tables_never_panic() {
    const MUTATIONS_PER_TABLE: usize = 60;
    const SEED: u64 = 0xC5F0_7A11_0B5E_ED01;
    let golden = common::golden_inputs();
    // The set of golden tables grows (the bus tables of WP-S1.21 D11 are the latest), so its total is not pinned
    // (CONTRIBUTING.md); the newest tables are mutated too.
    for table in ["logical_nets.csv", "physical_nets.csv", "bus_ports.csv"] {
        assert!(
            golden.iter().any(|i| i.name() == table),
            "{table} is a golden table"
        );
    }
    let importer = TablesImporter::new(common::library());
    let ctx = common::context();
    std::thread::scope(|scope| {
        for (n, target) in golden.iter().enumerate() {
            let (golden, importer, ctx) = (&golden, &importer, &ctx);
            scope.spawn(move || {
                let text = String::from_utf8(target.bytes().to_vec()).expect("golden tables are UTF-8");
                let mut rng = Lcg(SEED ^ ((n as u64 + 1) << 32));
                for _ in 0..MUTATIONS_PER_TABLE {
                    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
                    for _ in 0..=(rng.next() % 3) {
                        mutate_once(&mut lines, &mut rng);
                    }
                    let mut mutated = lines.join("\n");
                    mutated.push('\n');
                    let mut inputs: Vec<_> = golden
                        .iter()
                        .filter(|i| i.name() != target.name())
                        .cloned()
                        .collect();
                    inputs.push(
                        ew_interop_api::ImportInput::new(target.name(), mutated.clone().into_bytes())
                            .unwrap(),
                    );
                    if let Ok(batch) = importer.import(&inputs, ctx) {
                        let mut seen = std::collections::BTreeSet::new();
                        for r in &batch.records {
                            if let Some(id) = r.id() {
                                assert!(
                                    seen.insert(id),
                                    "{}: duplicate record ID {id:?}\n{mutated}",
                                    target.name()
                                );
                            }
                        }
                    }
                }
            });
        }
    });
}
