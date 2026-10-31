// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `open_items`: the open-items list (FR-REP-021; WP-S3.06 D7, D8).

use ew_checks::{Finding, OPEN_ITEM_RULES, open_item_of, open_items};
use ew_derive::ew_mltext::{NaturalKey, natural_key};
use ew_derive::ew_schema::ReportBlock;
use ew_derive::{PageRef, Q};

use super::conn::{put, text_cell};
use crate::kind::ReportKindDef;
use crate::queries::cycle_rows;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowSet};

/// The open-items list: one row per open item of `ew_checks::open_items` (the open findings of the open-item rules in
/// release mode of the project's release scheme, else `pre-delivery`), so a finding is listed exactly when `ew check
/// --mode release` prints it open (D7).
///
/// Order: the category's place in `OPEN_ITEM_RULES`, then the natural order of the item (the finding's subject), then
/// the fingerprint. Row ID `name(<rule>/<fingerprint>)`, which survives a renumber (the fingerprint holds no tag).
///
/// - `category`: the open-item category ("TBD value", "Device without part", ...).
/// - `item`: the finding's subject, as `ew check` prints it.
/// - `description`: the finding's message in en-GB, as `ew check` prints it (FP-4: the display language later).
/// - `rule`: the rule ID.
/// - `severity`: the effective release severity, `error`, `warning` or `info`.
/// - `page`: the finding's drawn page, empty when it has none.
///
/// A query cycle (a rule that reached the list through the page order, which D8's contract rules out) gives no rows,
/// `NotDerived::Cycle`, and a cycle note.
#[derive(Clone, Copy, Debug)]
pub struct OpenItems;

impl ReportKindDef for OpenItems {
    fn name(&self) -> &'static str {
        "open_items"
    }

    /// The weights are millimetres of the house table's 260.96 mm. Each short column holds its widest Golden A text at
    /// the 1.8 mm floor with the cell padding (probed: "Unconnected connection point" 37.46 mm,
    /// "=P01+J-E-2416-201U1:DP" 30.30, "EW-CORE-043" 17.24, "warning" 10.81, "Page" 6.97), and the description takes
    /// the rest. Two EW-CORE-043 messages of Golden A (184.95 and 164.31 mm at the floor) fit no split of the table:
    /// they print at the floor and are reported as overflow.
    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("category", CellType::Text, "Category").weighted(38),
            ColumnDef::new("item", CellType::Text, "Item").weighted(31),
            ColumnDef::new("description", CellType::Text, "Description").weighted(156),
            ColumnDef::new("rule", CellType::Ident, "Rule").weighted(18),
            ColumnDef::new("severity", CellType::Text, "Severity").weighted(11),
            ColumnDef::new("page", CellType::Folio, "Page").weighted(7),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let findings = match open_items(q) {
            Ok(findings) => findings,
            Err(cycle) => {
                return RowSet {
                    columns,
                    ..cycle_rows(q, cycle)
                };
            }
        };
        let mut keyed: Vec<((usize, NaturalKey, ew_checks::Fingerprint), Row)> = findings
            .iter()
            .filter_map(|f| {
                let row = open_item_of(f)?;
                let category = OPEN_ITEM_RULES
                    .iter()
                    .position(|o| o.category == row.category)
                    .unwrap_or(usize::MAX);
                Some((
                    (category, natural_key(&f.subject), f.fingerprint),
                    row_of(&columns, f, row.category),
                ))
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        let mut set = RowSet::new(columns);
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();
        set
    }
}

/// The row of one open item.
fn row_of(columns: &[ColumnDef], f: &Finding, category: &str) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    put(columns, &mut cells, "category", text_cell(category));
    put(columns, &mut cells, "item", text_cell(&f.subject));
    put(columns, &mut cells, "description", text_cell(&f.message));
    put(columns, &mut cells, "rule", Cell::Ident(f.rule.to_owned()));
    put(columns, &mut cells, "severity", text_cell(f.severity.name()));
    if let Some(page) = f.page {
        put(columns, &mut cells, "page", Cell::Folio(PageRef::Drawn(page)));
    }
    let id = RowId(vec![RowKey::Name(format!("{}/{}", f.rule, f.fingerprint))]);
    let mut r = Row::data(id, cells);
    r.sources = f.targets.iter().map(|t| t.id).collect();
    r
}
