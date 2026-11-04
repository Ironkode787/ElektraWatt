// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.08 part 2 over Golden A in `ew-api`: the round trip's session calls (`table_export`, `table_import`,
//! `apply_table_import`), where the undo step and the reader's bounds can be seen (RT-2, RT-6, D11).

use crate::golden_common;

use ew_api::output::{
    CellOutcome, CsvOptions, ImportTableOptions, Preset, TableCell, TableError, TableImportError, table_csv,
};
use ew_api::{Origin, Submit};

/// The key of -X1031's table and the table, in a fresh session over Golden A.
fn x1031() -> (ew_api::Session, String, ew_api::output::Table) {
    let session = golden_common::session_with_profiles(&golden_common::batch());
    let key = session.navigator_table_key("strips", Some("-X1031")).unwrap();
    let table = session.table_export(&key).unwrap();
    (session, key, table)
}

/// The texts of column `key` of `table`, row by row.
fn column(table: &ew_api::output::Table, key: &str) -> Vec<String> {
    let c = table.columns.iter().position(|c| c.key == key).unwrap();
    table.rows.iter().map(|r| r.cells[c].text().to_owned()).collect()
}

/// RT-2 (FR-EXP-004) and D11: five Function text cells changed in an `excel-en` CSV preview as five applied cells,
/// compile to one `fields.set_many`, change exactly those five labels when applied, and one undo restores the table.
#[test]
fn a_round_trip_applies_in_one_undo_step() {
    let (mut session, key, table) = x1031();
    assert_eq!(table.rows.len(), 20);
    let editable: Vec<&str> = table
        .columns
        .iter()
        .filter(|c| c.editable)
        .map(|c| c.key.as_str())
        .collect();
    assert_eq!(editable, ["terminal", "label", "state"]);
    let mut file = table.clone();
    let label = file.columns.iter().position(|c| c.key == "label").unwrap();
    for (i, row) in file.rows.iter_mut().take(5).enumerate() {
        row.cells[label] = TableCell::Text(format!("Valve {}", i + 1));
    }
    let csv = table_csv(&file, &CsvOptions::preset(Preset::ExcelEn));
    let options = ImportTableOptions {
        table: Some(key.clone()),
        ..ImportTableOptions::default()
    };
    let import = session.table_import(csv.as_bytes(), &options).unwrap();
    assert!(!import.three_way);
    let counts = import.counts();
    assert_eq!(
        (counts.changed, counts.kept, counts.conflicts, counts.refused),
        (5, 0, 0, 0)
    );
    assert!(
        import
            .cells
            .iter()
            .all(|c| c.outcome == CellOutcome::Applied && c.column == "label")
    );
    assert_eq!(
        import.command.as_ref().map(|c| c.name.as_str()),
        Some("fields.set_many")
    );
    let before = session.world().rev.0;
    session
        .apply_table_import(&import, Submit::from(Origin::Gui))
        .unwrap()
        .expect("a receipt");
    assert_eq!(session.world().rev.0, before + 1, "one transaction");
    let applied = session.table_export(&key).unwrap();
    let mut want = column(&table, "label");
    for (i, w) in want.iter_mut().take(5).enumerate() {
        *w = format!("Valve {}", i + 1);
    }
    assert_eq!(column(&applied, "label"), want);
    assert_eq!(column(&applied, "terminal"), column(&table, "terminal"));
    // The import read against an older revision is refused once the project moved.
    assert_eq!(
        session
            .apply_table_import(&import, Submit::from(Origin::Gui))
            .unwrap_err()
            .to_string(),
        "the project changed since the file was read: import the file again"
    );
    session.undo(&Origin::Gui).unwrap();
    assert_eq!(
        session.table_export(&key).unwrap(),
        table,
        "one undo step restores the table"
    );
}

/// D8, D11 and RT-6: the refusals of a round trip as whole texts; a file over the reader's limit is refused unread.
#[test]
fn the_round_trip_refuses_in_words() {
    let (session, key, table) = x1031();
    let csv = table_csv(&table, &CsvOptions::preset(Preset::ExcelEn));
    let refusal = |bytes: &[u8], table: Option<&str>| {
        let options = ImportTableOptions {
            table: table.map(str::to_owned),
            ..ImportTableOptions::default()
        };
        session.table_import(bytes, &options).unwrap_err().to_string()
    };
    assert_eq!(
        refusal(csv.as_bytes(), None),
        "a CSV names no table: give `--table <key>`"
    );
    assert_eq!(
        refusal(csv.as_bytes(), Some("sql")),
        "an `ew sql` result is never imported: import a table that `ew export table` or `ew export report` wrote"
    );
    assert_eq!(
        refusal(csv.as_bytes(), Some("nav:macros")),
        "unknown navigator `macros` (the table navigators are: pages, devices, connections, strips, cables, plc, \
         potentials, ips, structure, parts, blocks, unplaced)"
    );
    let golden = table_csv(&table, &CsvOptions::preset(Preset::Ew));
    assert_eq!(
        refusal(golden.as_bytes(), Some(&key)),
        "the file has no `id` column, by which its rows are matched: export the table as XLSX, or as CSV with \
         `--preset excel-nl`, `excel-en` or `rfc4180`, and keep the column"
    );
    let limit = ew_api::catalog::table_limits().max_input_bytes;
    let huge = vec![b'a'; usize::try_from(limit).unwrap() + 1];
    assert_eq!(
        session
            .table_import(&huge, &ImportTableOptions::default())
            .unwrap_err(),
        TableImportError::Read(format!("input exceeds the input size limit of {limit}"))
    );
    assert_eq!(
        refusal(&huge, Some(&key)),
        format!("the file cannot be read: input exceeds the input size limit of {limit}")
    );
    assert_eq!(
        session
            .navigator_table_key("devices", Some("-X1031"))
            .unwrap_err()
            .to_string(),
        "`--strip` applies to `export table strips`"
    );
    assert_eq!(
        session.navigator_table_key("strips", Some("-X9")).unwrap_err(),
        TableError::UnknownStrip(
            "-X9".to_owned(),
            vec!["-X1031".to_owned(), "-X1032".to_owned(), "1X1".to_owned()]
        )
    );
    assert_eq!(
        session
            .navigator_table_key("strips", Some("-X9"))
            .unwrap_err()
            .to_string(),
        "no terminal strip `-X9` (the strips are: -X1031, -X1032, 1X1)"
    );
}

/// SE-9 (WP-S6.04 D15, NFR-REL-12): a read that panics under `guard_table_read`, after it read the file against
/// the project, is S2 whole with the file's name, and the session's model is unchanged; a refusal keeps its own text.
#[test]
fn a_panicking_read_gives_s2_and_changes_nothing() {
    let (session, key, table) = x1031();
    let model = session.world().model.clone();
    let csv = table_csv(&table, &CsvOptions::preset(Preset::ExcelEn));
    let options = ImportTableOptions {
        table: Some(key),
        ..ImportTableOptions::default()
    };
    let e = ew_api::output::guard_table_read::<()>("x1031.csv", || {
        session.table_import(csv.as_bytes(), &options)?;
        panic!("a read that panics")
    })
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "The table import stopped on an internal error while reading x1031.csv; nothing was changed."
    );
    let e = ew_api::output::guard_table_read("x1031.csv", || {
        session.table_import(csv.as_bytes(), &ImportTableOptions::default())
    })
    .unwrap_err();
    assert_eq!(e, TableImportError::CsvNoTable);
    assert!(session.world().model.content_eq(&model), "the model changed");
}
