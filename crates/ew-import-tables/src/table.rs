// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tables by name, typed cell readers and the capped finding list.

use std::collections::BTreeMap;

use ew_interop_api::{Code, ReportEntry, Severity, SourceLocation};
use ew_mltext::{Ident, MlText, to_nfc};
use ew_schema::{ColourCode, ColourSpec, Csa, Date, IecColour};
use ew_units::Dec3;

use crate::MAX_REPORT_ENTRIES;
use crate::csv::CsvTable;

// ---------------------------------------------------------------------------------------------------------
// Findings

/// The findings of one import, capped at [`MAX_REPORT_ENTRIES`]: the rest is counted and summarised in one
/// EW-IMP-001 entry (doc 14 §11 "Report volume").
#[derive(Debug, Default)]
pub(crate) struct Findings {
    entries: Vec<ReportEntry>,
    suppressed: u64,
}

impl Findings {
    /// Records `entry`, or counts it once the cap is reached.
    pub(crate) fn push(&mut self, entry: ReportEntry) {
        if self.entries.len() < MAX_REPORT_ENTRIES {
            self.entries.push(entry);
        } else {
            self.suppressed += 1;
        }
    }

    /// A finding about row `key` (line `line`) of `file`.
    pub(crate) fn at(
        &mut self,
        code: Code,
        severity: Severity,
        file: &str,
        key: Option<&str>,
        line: Option<u32>,
        message: impl Into<String>,
    ) {
        self.push(ReportEntry::new(code, severity, message).at(SourceLocation {
            file: file.to_owned(),
            item: key.map(str::to_owned),
            line,
            ..SourceLocation::default()
        }));
    }

    /// The entries, with the summary of the suppressed ones appended.
    pub(crate) fn finish(mut self) -> Vec<ReportEntry> {
        if self.suppressed > 0 {
            self.entries.push(ReportEntry::new(
                Code::LIMIT,
                Severity::Warning,
                format!(
                    "{} further findings were not listed (at most {MAX_REPORT_ENTRIES} per import)",
                    self.suppressed
                ),
            ));
        }
        self.entries
    }
}

// ---------------------------------------------------------------------------------------------------------
// Tables

/// The column set of one known table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TableSpec {
    /// The file stem, also the `{table}` of record IDs.
    pub(crate) name: &'static str,
    /// Columns that must be present.
    pub(crate) required: &'static [&'static str],
    /// Columns that may be present.
    pub(crate) optional: &'static [&'static str],
}

/// One loaded table: header map and NFC rows.
#[derive(Debug)]
pub(crate) struct Table {
    /// The file name (`devices.csv`).
    pub(crate) file: String,
    columns: BTreeMap<String, usize>,
    rows: Vec<RowData>,
}

#[derive(Debug)]
struct RowData {
    line: u32,
    cells: Vec<String>,
}

impl Table {
    /// The table `csv` of `file` under `spec`, or `None` when a required column is missing (reported as
    /// EW-IMP-002). Unknown columns are reported (EW-IMP-004) and ignored. Every cell is NFC-normalised.
    pub(crate) fn new(spec: TableSpec, file: &str, csv: CsvTable, findings: &mut Findings) -> Option<Table> {
        let mut columns = BTreeMap::new();
        for (i, name) in csv.header.iter().enumerate() {
            if spec.required.contains(&name.as_str()) || spec.optional.contains(&name.as_str()) {
                columns.insert(name.clone(), i);
            } else {
                findings.at(
                    Code::DROPPED_BY_DESIGN,
                    Severity::Info,
                    file,
                    None,
                    Some(1),
                    format!("unknown column `{name}` ignored"),
                );
            }
        }
        let missing: Vec<&str> = spec
            .required
            .iter()
            .copied()
            .filter(|c| !columns.contains_key(*c))
            .collect();
        if !missing.is_empty() {
            findings.at(
                Code::MALFORMED_ITEM,
                Severity::Error,
                file,
                None,
                Some(1),
                format!(
                    "table refused: required column(s) missing: {}",
                    missing.join(", ")
                ),
            );
            return None;
        }
        let rows = csv
            .rows
            .into_iter()
            .map(|r| RowData {
                line: r.line,
                cells: r.cells.iter().map(|c| to_nfc(c).into_owned()).collect(),
            })
            .collect();
        Some(Table {
            file: file.to_owned(),
            columns,
            rows,
        })
    }

    /// The rows in file order.
    pub(crate) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        self.rows.iter().map(move |data| Row { table: self, data })
    }

    /// The number of rows.
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }
}

/// One row of a [`Table`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row<'t> {
    table: &'t Table,
    data: &'t RowData,
}

impl<'t> Row<'t> {
    /// The cell of `column`, `None` when the column is absent or the cell empty.
    pub(crate) fn get(&self, column: &str) -> Option<&'t str> {
        let i = *self.table.columns.get(column)?;
        self.data
            .cells
            .get(i)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// The `key` cell (empty when absent).
    pub(crate) fn key(&self) -> &'t str {
        self.get("key").unwrap_or("")
    }

    /// The 1-based line of the row.
    pub(crate) fn line(&self) -> u32 {
        self.data.line
    }

    /// The file name.
    pub(crate) fn file(&self) -> &'t str {
        &self.table.file
    }

    /// A reader of this row's cells that remembers the first problem.
    pub(crate) fn reader(self) -> Rd<'t> {
        Rd { row: self, err: None }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Typed cells

/// A typed reader of one row. Every getter returns `None` for an empty cell; a required getter (`req_*`) or
/// a malformed cell records the first problem, which [`Rd::finish`] returns.
#[derive(Debug)]
pub(crate) struct Rd<'t> {
    row: Row<'t>,
    err: Option<String>,
}

impl<'t> Rd<'t> {
    /// Records a problem (the first one wins).
    pub(crate) fn fail(&mut self, message: impl Into<String>) {
        if self.err.is_none() {
            self.err = Some(message.into());
        }
    }

    /// The first problem, if any.
    pub(crate) fn finish(self) -> Result<(), String> {
        match self.err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    /// The raw text of `col`.
    pub(crate) fn text(&mut self, col: &str) -> Option<&'t str> {
        self.row.get(col)
    }

    /// The raw text of `col`; a problem when empty.
    pub(crate) fn req(&mut self, col: &str) -> Option<&'t str> {
        let v = self.row.get(col);
        if v.is_none() {
            self.fail(format!("column `{col}` is empty"));
        }
        v
    }

    fn parse<T>(
        &mut self,
        col: &str,
        v: Option<&'t str>,
        f: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        match f(v?) {
            Ok(t) => Some(t),
            Err(e) => {
                self.fail(format!("column `{col}`: {e}"));
                None
            }
        }
    }

    /// An identifier.
    pub(crate) fn ident(&mut self, col: &str) -> Option<Ident> {
        let v = self.text(col);
        self.parse(col, v, parse_ident)
    }

    /// A required identifier.
    pub(crate) fn req_ident(&mut self, col: &str) -> Option<Ident> {
        let v = self.req(col);
        self.parse(col, v, parse_ident)
    }

    /// Language-neutral text (empty when absent).
    pub(crate) fn mltext(&mut self, col: &str) -> MlText {
        self.text(col).map(MlText::neutral).unwrap_or_default()
    }

    /// An unsigned integer.
    pub(crate) fn uint<T: TryFrom<u64>>(&mut self, col: &str) -> Option<T> {
        let v = self.text(col);
        self.parse(col, v, parse_uint)
    }

    /// A required unsigned integer.
    pub(crate) fn req_uint<T: TryFrom<u64>>(&mut self, col: &str) -> Option<T> {
        let v = self.req(col);
        self.parse(col, v, parse_uint)
    }

    /// A signed integer.
    pub(crate) fn int(&mut self, col: &str) -> Option<i64> {
        let v = self.text(col);
        self.parse(col, v, |s| {
            s.parse::<i64>().map_err(|_| format!("`{s}` is not an integer"))
        })
    }

    /// A decimal.
    pub(crate) fn dec3(&mut self, col: &str) -> Option<Dec3> {
        let v = self.text(col);
        self.parse(col, v, parse_dec3)
    }

    /// A boolean (`true` / `false`; empty is false).
    pub(crate) fn flag(&mut self, col: &str) -> bool {
        let v = self.text(col);
        self.parse(col, v, |s| match s {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("`{s}` is not true or false")),
        })
        .unwrap_or(false)
    }

    /// A date `YYYY-MM-DD`.
    pub(crate) fn date(&mut self, col: &str) -> Option<Date> {
        let v = self.text(col);
        self.parse(col, v, |s| s.parse::<Date>().map_err(|e| format!("`{s}`: {e}")))
    }

    /// A `;`-separated list (empty when absent).
    pub(crate) fn list(&mut self, col: &str) -> Vec<&'t str> {
        match self.text(col) {
            None => Vec::new(),
            Some(s) => {
                let items: Vec<&str> = s.split(';').collect();
                if items.iter().any(|i| i.is_empty() || i.trim() != *i) {
                    self.fail(format!("column `{col}`: malformed list `{s}`"));
                }
                items
            }
        }
    }

    /// One of the names of `table`.
    pub(crate) fn choice<T: Clone>(&mut self, col: &str, table: &[(&str, T)]) -> Option<T> {
        let v = self.text(col);
        self.parse(col, v, |s| {
            table
                .iter()
                .find(|(n, _)| *n == s)
                .map(|(_, t)| t.clone())
                .ok_or_else(|| {
                    let names: Vec<&str> = table.iter().map(|(n, _)| *n).collect();
                    format!("`{s}` is not one of {}", names.join(", "))
                })
        })
    }

    /// A required choice.
    pub(crate) fn req_choice<T: Clone>(&mut self, col: &str, table: &[(&str, T)]) -> Option<T> {
        if self.row.get(col).is_none() {
            self.fail(format!("column `{col}` is empty"));
            return None;
        }
        self.choice(col, table)
    }

    /// A colour cell (`BK`, `GNYE`, `DBU`).
    pub(crate) fn colour(&mut self, col: &str) -> Option<ColourSpec> {
        let v = self.text(col);
        self.parse(col, v, parse_colour)
    }

    /// A cross-section in mm².
    pub(crate) fn csa(&mut self, col: &str) -> Option<Csa> {
        self.dec3(col).map(Csa::Mm2)
    }
}

/// An identifier from a cell (already NFC and trimmed by the dialect).
pub(crate) fn parse_ident(s: &str) -> Result<Ident, String> {
    s.parse::<Ident>()
        .map_err(|e| format!("`{s}` is not an identifier: {e}"))
}

fn parse_uint<T: TryFrom<u64>>(s: &str) -> Result<T, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("`{s}` is not an unsigned integer"));
    }
    let n: u64 = s.parse().map_err(|_| format!("`{s}` is out of range"))?;
    T::try_from(n).map_err(|_| format!("`{s}` is out of range"))
}

/// A canonical `Dec3` cell.
pub(crate) fn parse_dec3(s: &str) -> Result<Dec3, String> {
    s.parse::<Dec3>()
        .map_err(|e| format!("`{s}` is not a decimal: {e}"))
}

/// A colour cell: a run of IEC 60757 two-letter codes (`BK`, `GNYE` = GN + YE), else one company code (`DBU`).
pub(crate) fn parse_colour(s: &str) -> Result<ColourSpec, String> {
    let bytes = s.as_bytes();
    if s.is_ascii() && bytes.len() % 2 == 0 && !bytes.is_empty() {
        let codes: Option<Vec<ColourCode>> = bytes
            .chunks(2)
            .map(|c| {
                std::str::from_utf8(c)
                    .ok()
                    .and_then(|c| c.parse::<IecColour>().ok())
                    .map(ColourCode::Iec)
            })
            .collect();
        if let Some(codes) = codes {
            return ColourSpec::new(codes).map_err(|e| format!("`{s}`: {e}"));
        }
    }
    let code = parse_ident(s)?;
    let code = ColourCode::company(code).map_err(|e| format!("`{s}`: {e}"))?;
    Ok(ColourSpec::one(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(
            parse_colour("BK").unwrap().codes(),
            [ColourCode::Iec(IecColour::Bk)]
        );
        assert_eq!(
            parse_colour("GNYE").unwrap().codes(),
            [ColourCode::Iec(IecColour::Gn), ColourCode::Iec(IecColour::Ye)]
        );
        assert!(matches!(
            parse_colour("DBU").unwrap().codes(),
            [ColourCode::Company(_)]
        ));
    }

    #[test]
    fn integers() {
        assert_eq!(parse_uint::<u32>("12"), Ok(12));
        assert!(parse_uint::<u8>("300").is_err());
        assert!(parse_uint::<u32>("-1").is_err());
        assert!(parse_uint::<u32>("1.0").is_err());
    }
}
