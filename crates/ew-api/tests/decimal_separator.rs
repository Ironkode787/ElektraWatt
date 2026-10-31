// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-366: `settings.formatting.decimal_separator` reaches the rendering of decimals in the printed tables of the
//! generated pages (`ew_reports::render`, neutral language). The golden project sets `point`, a project that sets
//! nothing prints the point as well, `comma` prints the comma.

mod golden_common;

use ew_api::Session;
use ew_api::output::PageTableOf;
use ew_derive::PageOrder;
use ew_schema::DecimalSeparator;

/// Every printed cell of every generated page's table, in page order.
fn table_cells(s: &Session) -> Vec<String> {
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let mut cells = Vec::new();
    for e in &order.entries {
        if let Some(table) = q.get::<PageTableOf>(e.page).as_ref() {
            cells.extend(table.rows.iter().flat_map(|r| r.cells.iter().cloned()));
        }
    }
    cells
}

fn with_separator(separator: Option<DecimalSeparator>) -> Vec<String> {
    let batch = golden_common::batch();
    let s = golden_common::session_with(&batch, |project, _| {
        project.settings.formatting.decimal_separator = separator;
    });
    table_cells(&s)
}

#[test]
fn the_project_decimal_separator_prints_in_the_tables_of_the_generated_pages() {
    let point = with_separator(Some(DecimalSeparator::Point));
    let none = with_separator(None);
    let comma = with_separator(Some(DecimalSeparator::Comma));
    // The golden cable list prints "0.75 mm²" and the bill of materials the lengths "1.71" and "3.17" (metres, the
    // carrier part lines of -UC1).
    for printed in ["0.75 mm²", "1.5 mm²", "1.71", "3.17"] {
        assert!(
            point.iter().any(|c| c == printed),
            "{printed} in the point output"
        );
        assert!(
            !comma.iter().any(|c| c == printed),
            "{printed} in the comma output"
        );
    }
    assert_eq!(
        point, none,
        "no setting prints as `point` does (the golden output)"
    );
    // Only the decimals differ: the same cells, each equal or equal after the separator.
    assert_eq!(comma.len(), point.len());
    let differing: Vec<(&String, &String)> = point.iter().zip(&comma).filter(|(a, b)| a != b).collect();
    assert!(differing.len() > 50, "{} cells differ", differing.len());
    for (a, b) in differing {
        assert_eq!(&a.replacen('.', ",", 1), b);
    }
}
