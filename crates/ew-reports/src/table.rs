// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one table model of the table writers (WP-S3.08 D4): report rows, the `ew sql` result and, from part 2, the
//! navigator tables are a [`Table`], which the XLSX writer ([`crate::xlsx`]) and the CSV presets
//! ([`crate::csv::table_csv`]) write.
//!
//! A table has a name (its sheet's), a key (what it is: `block:<uuid>`, `kind:<name>`, `sql`; WP-S3.08 D8), typed
//! columns and rows. A row carries its [`RowId`](crate::RowId) text (the hidden `id` column of the round trip), whether
//! it is a heading, and one cell per column. A cell is text, a number or a date, each as its neutral printed text:
//! a number with a `.` decimal point and no unit, a date as `yyyy-mm-dd`, so the writers can type them (a numeric cell,
//! a date serial) and the Excel presets can write a decimal comma.
//!
//! [`Table::from_rowset`] prints every cell as the folio does ([`crate::render::render`], neutral language), except a
//! number, a quantity without a unit and a date, which take their neutral form: a quantity with a unit (`1.5 mm²`)
//! stays text, as do a missing or stale value in a number column.

use ew_derive::Q;

use crate::render::{RenderOptions, decimal, render};
use crate::rowset::{Cell, CellType, QtyUnit, RowKind, RowSet};

/// One table: what a sheet of a workbook, or one CSV file, holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Table {
    /// The name: the sheet's name in a workbook (made a valid sheet name by the writer).
    pub name: String,
    /// What the table is (WP-S3.08 D8): `block:<uuid>`, `kind:<name>`, `kind:parts_summary/<mode>` or `sql`.
    pub key: String,
    /// The columns, in order (the `id` column is not one of them: every writer puts it first).
    pub columns: Vec<TableColumn>,
    /// The rows, in order.
    pub rows: Vec<TableRow>,
}

/// One column of a [`Table`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableColumn {
    /// The key (`{row.<key>}` of the report's columns, a SQL result's column name).
    pub key: String,
    /// The header label, neutral language.
    pub label: String,
    /// The value type of the column.
    pub cell: CellType,
    /// Whether a round trip may change the column's cells (WP-S3.08 D9; every report column is read-only in part 1).
    pub editable: bool,
}

/// One row of a [`Table`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableRow {
    /// The row's ID text ([`RowId`](crate::RowId)'s text form); `None` for a heading row and for a row without an
    /// identity (a SQL result's).
    pub id: Option<String>,
    /// Whether the row is a heading (a group header or footer, a heading inside the rows).
    pub heading: bool,
    /// One cell per column.
    pub cells: Vec<TableCell>,
}

/// One cell of a [`TableRow`], as its neutral printed text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableCell {
    /// Text, written as text everywhere (never a formula).
    Text(String),
    /// A number: an optional `-`, digits, an optional `.` and digits (`1.71`, `-3`, `28`).
    Number(String),
    /// A date, `yyyy-mm-dd`.
    Date(String),
}

impl TableCell {
    /// The cell's text.
    pub fn text(&self) -> &str {
        match self {
            TableCell::Text(t) | TableCell::Number(t) | TableCell::Date(t) => t,
        }
    }
}

impl Table {
    /// The table of a row set (WP-S3.08 D4): its columns with their neutral labels (every column read-only), and each
    /// row with its ID (none for a heading) and its cells as the module documentation says.
    pub fn from_rowset(q: &Q, set: &RowSet, name: &str, key: &str) -> Table {
        let options = RenderOptions::default();
        Table {
            name: name.to_owned(),
            key: key.to_owned(),
            columns: set
                .columns
                .iter()
                .map(|c| TableColumn {
                    key: c.key.clone(),
                    label: c.label.as_neutral().unwrap_or(&c.key).to_owned(),
                    cell: c.ty,
                    editable: false,
                })
                .collect(),
            rows: set
                .rows
                .iter()
                .map(|r| {
                    let heading = r.kind != RowKind::Data;
                    TableRow {
                        id: (!heading).then(|| r.id.to_string()),
                        heading,
                        cells: r.cells.iter().map(|c| table_cell(q, c, &options)).collect(),
                    }
                })
                .collect(),
        }
    }

    /// The number of data rows (headings excluded).
    pub fn data_rows(&self) -> usize {
        self.rows.iter().filter(|r| !r.heading).count()
    }
}

/// One cell of a row set as a table cell.
fn table_cell(q: &Q, cell: &Cell, options: &RenderOptions) -> TableCell {
    match cell {
        Cell::Number(n) => TableCell::Number(n.to_string()),
        Cell::Qty(v, QtyUnit::None) => TableCell::Number(decimal(*v, false)),
        Cell::Date(d) => TableCell::Date(d.to_string()),
        other => TableCell::Text(render(q, other, options)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_gives_its_text() {
        assert_eq!(TableCell::Text("-3X1".into()).text(), "-3X1");
        assert_eq!(TableCell::Number("1.71".into()).text(), "1.71");
        assert_eq!(TableCell::Date("2026-01-31".into()).text(), "2026-01-31");
    }
}
