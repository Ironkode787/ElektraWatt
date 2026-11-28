// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `settings.formatting.decimal_separator` reaches the rendering of decimals in the printed tables of the
//! generated pages (`ew_reports::render`, neutral language), on the sample project: `point` and a project that sets
//! nothing print the point, `comma` prints the comma.

use crate::common;

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
    let (mut project, records, lib) = common::sample_records();
    project.settings.formatting.decimal_separator = separator;
    table_cells(&common::session_of(project, records, lib))
}

#[test]
fn the_project_decimal_separator_prints_in_the_tables_of_the_generated_pages() {
    let point = with_separator(Some(DecimalSeparator::Point));
    let none = with_separator(None);
    let comma = with_separator(Some(DecimalSeparator::Comma));
    // The wire list prints the cross-sections "0.75" and "1.5" under mm², the parts list the lengths "0.94" and "0.47"
    // (metres of the plate's rails and duct).
    for printed in ["0.75", "1.5", "0.94", "0.47"] {
        assert!(
            point.iter().any(|c| c == printed),
            "{printed} in the point output"
        );
        assert!(
            !comma.iter().any(|c| c == printed),
            "{printed} in the comma output"
        );
    }
    assert_eq!(point, none, "no setting prints as `point` does");
    // Only the decimals differ: the same cells, each equal or equal after the separator (a strip's heading in the
    // assembly list holds two measured lengths); a twin ferrule's partner after " · " is a wire number and keeps its
    // point ("twin 2,5 BU · 101.6"), as does a wire that joins a clamp on site ("ferrule 0,75 GY · 104.8 joins it on
    // site").
    assert_eq!(comma.len(), point.len());
    let differing: Vec<(&String, &String)> = point.iter().zip(&comma).filter(|(a, b)| a != b).collect();
    assert!(!differing.is_empty());
    for (a, b) in differing {
        let named = a.starts_with("twin ") || a.ends_with(" joins it on site");
        let (size, partner) = match a.split_once(" · ").filter(|_| named) {
            Some((s, p)) => (s, Some(p)),
            None => (a.as_str(), None),
        };
        let want = match partner {
            Some(p) => format!("{} · {p}", size.replace('.', ",")),
            None => size.replace('.', ","),
        };
        assert_eq!(&want, b);
    }
}

/// The hole table's heading names its surface's size with the project's decimal sign, as its rows' numbers: a door
/// half a millimetre wider prints "600,5 × 800 mm" with the comma.
#[test]
fn the_hole_table_heading_takes_the_decimal_sign() {
    let (mut project, mut records, lib) = common::sample_records();
    project.settings.formatting.decimal_separator = Some(DecimalSeparator::Comma);
    for r in &mut records {
        if let ew_schema::Record::MountingSurface(m) = r
            && m.kind == ew_schema::SurfaceKind::Door
        {
            let right = m.usable.iter().map(|p| p.x.0).max().unwrap_or_default();
            for p in &mut m.usable {
                if p.x.0 == right {
                    p.x.0 += 500_000;
                }
            }
        }
    }
    let cells = table_cells(&common::session_of(project, records, lib));
    assert!(cells.iter().any(|c| c.contains("600,5 × 800 mm")), "{cells:#?}");
}
