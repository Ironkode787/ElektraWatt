// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table writers behind the `Exporter` trait (WP-S3.09 D14, decision log ID-1016): [`CsvExporter`] (the presets
//! of [`crate::csv::table_csv`]) and [`XlsxExporter`] ([`crate::xlsx::xlsx`]) write the [`RowTable`]s of an
//! [`ExportView`], each converted to this crate's [`Table`] and back without loss, so the bytes are those the writers
//! give a [`Table`] directly. [`export_tables`] runs an exporter over tables of this crate.

use std::collections::BTreeMap;

use ew_interop_api::{
    CellKind, ExportContext, ExportError, ExportReport, ExportView, Exporter, OutputMeta, RowCell, RowColumn, RowTable,
    RowTableRow,
};

use crate::csv::{CsvOptions, table_csv};
use crate::rowset::CellType;
use crate::table::{Table, TableCell, TableColumn, TableRow};
use crate::xlsx::{WorkbookMeta, cell_type_name, xlsx};

/// The CSV presets as an exporter: the one table of the view in the options' preset.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CsvExporter {
    /// The preset, byte-order mark and line end.
    pub options: CsvOptions,
}

/// The XLSX writer as an exporter: every table of the view as a sheet, in key order, with the workbook's metadata.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XlsxExporter {
    /// The workbook's metadata; its `rev` is the view's.
    pub meta: WorkbookMeta,
}

impl Exporter for CsvExporter {
    fn id(&self) -> &'static str {
        "csv"
    }

    fn export(&self, view: &ExportView<'_>, sink: &mut dyn std::io::Write) -> Result<ExportReport, ExportError> {
        let mut tables = view.rows.values();
        let (Some(table), None) = (tables.next(), tables.next()) else {
            return Err(ExportError::Refused(format!(
                "a CSV holds one table; the export has {}",
                view.rows.len()
            )));
        };
        let text = table_csv(&table_of(table), &self.options);
        write(sink, text.as_bytes())?;
        Ok(ExportReport {
            rev: view.rev,
            outputs: vec![OutputMeta::of(&table.name, "text/csv", text.as_bytes())],
            findings: Vec::new(),
        })
    }
}

impl Exporter for XlsxExporter {
    fn id(&self) -> &'static str {
        "xlsx"
    }

    fn export(&self, view: &ExportView<'_>, sink: &mut dyn std::io::Write) -> Result<ExportReport, ExportError> {
        let tables: Vec<Table> = view.rows.values().map(table_of).collect();
        let meta = WorkbookMeta {
            rev: view.rev,
            ..self.meta.clone()
        };
        let bytes = xlsx(&tables, &meta).map_err(|e| ExportError::Refused(e.to_string()))?;
        write(sink, &bytes)?;
        let name = tables.first().map_or("Table", |t| t.name.as_str());
        Ok(ExportReport {
            rev: view.rev,
            outputs: vec![OutputMeta::of(
                name,
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                &bytes,
            )],
            findings: Vec::new(),
        })
    }
}

/// `bytes` written to `sink`.
fn write(sink: &mut dyn std::io::Write, bytes: &[u8]) -> Result<(), ExportError> {
    sink.write_all(bytes).map_err(|e| ExportError::Write(e.to_string()))
}

/// The output of `exporter` over `tables` (in order) at revision `rev`: its bytes and its report.
///
/// # Errors
///
/// As the exporter's [`Exporter::export`].
pub fn export_tables(
    exporter: &dyn Exporter,
    rev: u64,
    tables: &[Table],
) -> Result<(Vec<u8>, ExportReport), ExportError> {
    let rows: BTreeMap<String, RowTable> = tables
        .iter()
        .enumerate()
        .map(|(i, t)| (format!("{i:06}"), row_table(t)))
        .collect();
    let view = ExportView {
        rev,
        records: &[],
        rows: &rows,
        context: ExportContext::default(),
    };
    let mut bytes = Vec::new();
    let report = exporter.export(&view, &mut bytes)?;
    Ok((bytes, report))
}

/// `table` as the trait's plain-data table.
pub fn row_table(table: &Table) -> RowTable {
    RowTable {
        name: table.name.clone(),
        key: table.key.clone(),
        columns: table
            .columns
            .iter()
            .map(|c| RowColumn {
                key: c.key.clone(),
                label: c.label.clone(),
                kind: cell_type_name(c.cell).to_owned(),
                editable: c.editable,
            })
            .collect(),
        rows: table
            .rows
            .iter()
            .map(|r| RowTableRow {
                id: r.id.clone(),
                heading: r.heading,
                cells: r
                    .cells
                    .iter()
                    .map(|c| RowCell {
                        text: c.text().to_owned(),
                        kind: match c {
                            TableCell::Text(_) => CellKind::Text,
                            TableCell::Number(_) => CellKind::Number,
                            TableCell::Date(_) => CellKind::Date,
                        },
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The trait's plain-data table as a [`Table`]; a column kind this crate does not name is text.
pub fn table_of(table: &RowTable) -> Table {
    Table {
        name: table.name.clone(),
        key: table.key.clone(),
        columns: table
            .columns
            .iter()
            .map(|c| TableColumn {
                key: c.key.clone(),
                label: c.label.clone(),
                cell: cell_type(&c.kind),
                editable: c.editable,
            })
            .collect(),
        rows: table
            .rows
            .iter()
            .map(|r| TableRow {
                id: r.id.clone(),
                heading: r.heading,
                cells: r
                    .cells
                    .iter()
                    .map(|c| match c.kind {
                        CellKind::Text => TableCell::Text(c.text.clone()),
                        CellKind::Number => TableCell::Number(c.text.clone()),
                        CellKind::Date => TableCell::Date(c.text.clone()),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The [`CellType`] whose [`cell_type_name`] is `name`; [`CellType::Text`] for another name.
fn cell_type(name: &str) -> CellType {
    const ALL: [CellType; 13] = [
        CellType::Text,
        CellType::Ident,
        CellType::Number,
        CellType::Tag,
        CellType::Folio,
        CellType::Folios,
        CellType::CellRef,
        CellType::Qty,
        CellType::Colour,
        CellType::Address,
        CellType::Date,
        CellType::Bool,
        CellType::Range,
    ];
    ALL.into_iter()
        .find(|t| cell_type_name(*t) == name)
        .unwrap_or(CellType::Text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::Preset;
    use crate::xlsx::DateTime;

    fn table() -> Table {
        let column = |key: &str, cell: CellType, editable: bool| TableColumn {
            key: key.into(),
            label: key.to_uppercase(),
            cell,
            editable,
        };
        Table {
            name: "Wires".into(),
            key: "kind:wire_list".into(),
            columns: vec![
                column("tag", CellType::Tag, false),
                column("len", CellType::Number, true),
                column("on", CellType::Date, false),
            ],
            rows: vec![
                TableRow {
                    id: None,
                    heading: true,
                    cells: vec![TableCell::Text("-X1".into()), TableCell::Text(String::new()), TableCell::Text(String::new())],
                },
                TableRow {
                    id: Some("e:1".into()),
                    heading: false,
                    cells: vec![
                        TableCell::Text("=-K1".into()),
                        TableCell::Number("1.5".into()),
                        TableCell::Date("2026-01-31".into()),
                    ],
                },
            ],
        }
    }

    /// D14: a table goes to the trait's plain data and back unchanged, every column kind included.
    #[test]
    fn a_table_converts_without_loss() {
        let t = table();
        assert_eq!(table_of(&row_table(&t)), t);
        for name in ["Text", "Ident", "Number", "Tag", "Folio", "Folios", "CellRef", "Qty", "Colour", "Address", "Date", "Bool", "Range"] {
            assert_eq!(cell_type_name(cell_type(name)), name);
        }
    }

    /// IC-15: the CSV and XLSX writers give the same bytes through the trait as directly, and the report names the
    /// revision and the output's digest.
    #[test]
    fn the_writers_give_the_same_bytes_through_the_trait() {
        let t = table();
        for preset in [Preset::ExcelNl, Preset::ExcelEn, Preset::Rfc4180, Preset::Ew] {
            let options = CsvOptions::preset(preset);
            let (bytes, report) = export_tables(&CsvExporter { options }, 7, std::slice::from_ref(&t)).unwrap();
            assert_eq!(bytes, table_csv(&t, &options).into_bytes());
            assert_eq!(report.rev, 7);
            assert_eq!(report.outputs, vec![OutputMeta::of("Wires", "text/csv", &bytes)]);
        }
        let meta = WorkbookMeta {
            date: DateTime {
                year: 2026,
                month: 1,
                day: 31,
                hour: 12,
                minute: 0,
                second: 0,
            },
            project: "p".into(),
            rev: 7,
            generation: Some(3),
            version: "0.1.0".into(),
            base_sha256: Some("00".repeat(32)),
        };
        let tables = [t.clone(), Table { name: "A".into(), ..t }];
        let (bytes, report) = export_tables(&XlsxExporter { meta: meta.clone() }, 7, &tables).unwrap();
        assert_eq!(bytes, xlsx(&tables, &meta).unwrap());
        assert_eq!(report.rev, 7);
        assert_eq!(report.outputs.len(), 1);
    }

    /// A CSV holds one table: two are refused in words.
    #[test]
    fn a_csv_of_two_tables_is_refused() {
        let t = table();
        let err = export_tables(&CsvExporter { options: CsvOptions::default() }, 1, &[t.clone(), t]).unwrap_err();
        assert_eq!(err.to_string(), "a CSV holds one table; the export has 2");
    }
}
