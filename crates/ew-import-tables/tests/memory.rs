// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Memory of the CSV path (doc 14 §11): cells are spans into the input, the cell budget refuses a table that
//! would cost too much, and the project ID of the context is checked against the one the tables derive.

mod common;

use ew_ids::Id;
use ew_import_tables::csv::{self, CsvError, TableLimits};
use ew_import_tables::{TablesImporter, bootstrap_context, ids};
use ew_interop_api::ImportError;

/// A table of 1,024 columns: header `key,c1,…,c1023`, then `rows` rows of a short key and only commas, so every
/// cell is one input byte.
fn comma_table(rows: usize) -> String {
    let mut text = String::from("key");
    for c in 1..1_024 {
        text.push_str(&format!(",c{c}"));
    }
    text.push('\n');
    let commas = ",".repeat(1_023);
    for r in 0..rows {
        text.push_str(&format!("k{r:04}{commas}\n"));
    }
    text
}

/// A comma-heavy table of about 4 MiB stays within the documented bound: every cell is an 8-byte span and every
/// cell costs at least one input byte, so the table's storage is at most 12 times the input (the bound for
/// cells of up to 12 bytes, plus the side buffer and the header, in 13 times the input and 64 KiB); this
/// encoding needs 8 per cell.
#[test]
fn a_comma_heavy_table_stays_within_the_memory_bound() {
    let text = comma_table(4_000);
    let len = text.len();
    assert!(
        (4 << 20) - (200 << 10) < len && len < (4 << 20) + (200 << 10),
        "{len}"
    );
    // About 4.1 million cells: raise the budget above the default 4,000,000 for this test.
    let limits = TableLimits {
        max_cells: 8_000_000,
        ..TableLimits::default()
    };
    let t = csv::parse(text.as_bytes(), &limits).unwrap();
    assert_eq!(t.row_count(), 4_000);
    assert_eq!(t.column_count(), 1_024);
    assert_eq!(t.cell(text.as_bytes(), 3_999, 0), "k3999");
    assert_eq!(t.cell(text.as_bytes(), 3_999, 1_023), "");

    let accounted = t.accounted_bytes();
    assert!(
        accounted <= 13 * len + 64 * 1024,
        "{accounted} bytes accounted for {len} input bytes"
    );
    // This encoding is 8 bytes per cell and cells are at most one per input byte: the figure is far below the
    // specified bound.
    assert!(
        accounted <= 9 * len + 64 * 1024,
        "{accounted} bytes accounted for {len} input bytes"
    );
}

/// The budget counts header cells and refuses at the cell above it; the default budget is 4,000,000 cells.
#[test]
fn the_cell_budget_is_exact() {
    assert_eq!(TableLimits::default().max_cells, 4_000_000);
    let limits = |max_cells| TableLimits {
        max_cells,
        ..TableLimits::default()
    };
    let text = b"a,b\n1,2\n3,4\n";
    assert!(csv::parse(text, &limits(6)).is_ok());
    assert_eq!(
        csv::parse(text, &limits(5)),
        Err(CsvError::TooManyCells { limit: 5 })
    );
}

/// The importer turns the cell budget into `LimitExceeded`.
#[test]
fn the_importer_refuses_a_table_above_the_cell_budget() {
    let importer = TablesImporter::new(common::library()).with_limits(TableLimits {
        max_cells: 3,
        ..TableLimits::default()
    });
    let inputs = common::inputs(&[("project.csv", "key,derivation_profile\nJ-E-2416,2027.1\n")]);
    let r = importer.import(&inputs, &common::context());
    match r {
        Err(ImportError::LimitExceeded { what, limit }) => {
            assert!(what.contains("cell count"), "{what}");
            assert_eq!(limit, 3);
        }
        other => panic!("{other:?}"),
    }
}

/// The bootstrap context carries the 64 MiB preset and a nil project.
#[test]
fn the_bootstrap_context_uses_the_bootstrap_limits() {
    let ctx = bootstrap_context();
    assert_eq!(ctx.limits, ew_interop_api::Limits::bootstrap_tables());
    assert_eq!(ctx.limits.max_input_bytes, 64 * 1024 * 1024);
    assert_eq!(ctx.project, Id::from_uuid(ew_ids::Uuid::nil()));
}

fn import_into(project: Id<ew_schema::Project>) -> Result<(), ImportError> {
    let mut ctx = common::context();
    ctx.project = project;
    let inputs = common::inputs(&[("project.csv", common::PROJECT_CSV)]);
    TablesImporter::new(common::library())
        .import(&inputs, &ctx)
        .map(|_| ())
}

/// `ctx.project` must be nil or the derived ID.
#[test]
fn ctx_project_must_be_nil_or_the_derived_id() {
    let derived = ids::project_id("J-E-2416");
    import_into(Id::from_uuid(ew_ids::Uuid::nil())).expect("nil is accepted");
    import_into(derived).expect("the derived ID is accepted");

    let other = Id::from_uuid(ew_ids::Uuid::from_u128(0x1234));
    assert_ne!(other, derived);
    match import_into(other) {
        Err(ImportError::Malformed { message, line }) => {
            assert_eq!(line, Some(2));
            assert!(
                message.starts_with("EW-IMP-002: the import context targets project "),
                "{message}"
            );
            assert!(message.contains(&other.to_string()), "{message}");
            assert!(message.contains(&derived.to_string()), "{message}");
            assert!(message.contains("`J-E-2416`"), "{message}");
            assert!(
                message.ends_with("the bootstrap creates its own project"),
                "{message}"
            );
        }
        r => panic!("{r:?}"),
    }
}
