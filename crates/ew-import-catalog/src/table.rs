// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two table readers (D11): CSV (UTF-8 with or without a byte-order mark, `,` or `;` found from the header line,
//! RFC 4180 quoting) and XLSX ([`crate::xlsx`], the first or the named sheet, each cell as its displayed text; a formula's
//! cached value is read and never evaluated). Both give a [`Sheet`]: the header row and the data rows with their
//! 1-based line (CSV) or row number (XLSX).

use ew_interop_api::{ImportError, Limits};

/// The most data rows of one table.
pub const MAX_ROWS: usize = 1_000_000;

/// The most columns of one table.
pub const MAX_COLUMNS: usize = 1_024;

/// The format of a parts table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceFormat {
    /// Comma- or semicolon-separated text.
    Csv,
    /// An Office Open XML workbook.
    Xlsx,
}

impl SourceFormat {
    /// The format's name in the report (`csv`, `xlsx`).
    pub fn name(self) -> &'static str {
        match self {
            SourceFormat::Csv => "csv",
            SourceFormat::Xlsx => "xlsx",
        }
    }

    /// The format of a file starting with `head`: a ZIP archive is a workbook, anything else is read as CSV.
    pub fn detect(head: &[u8]) -> SourceFormat {
        if head.starts_with(b"PK\x03\x04") {
            SourceFormat::Xlsx
        } else {
            SourceFormat::Csv
        }
    }
}

/// A cell: its text, or the error value a workbook stored (`#N/A`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum Cell {
    Text(String),
    Error(String),
}

impl Cell {
    pub(crate) fn text(&self) -> &str {
        match self {
            Cell::Text(t) => t,
            Cell::Error(_) => "",
        }
    }
}

/// A data row: its line or row number and its cells, one per header column (missing cells are empty).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Row {
    pub line: u32,
    pub cells: Vec<Cell>,
}

/// A table: the header row and the data rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct Sheet {
    pub headers: Vec<String>,
    pub rows: Vec<Row>,
    /// Data rows with cells beyond the last header (their values are not read).
    pub overhang: usize,
}

pub(crate) fn malformed(message: impl Into<String>, line: Option<u32>) -> ImportError {
    ImportError::Malformed {
        message: message.into(),
        line,
    }
}

pub(crate) fn too_many(what: &str, limit: usize) -> ImportError {
    ImportError::LimitExceeded {
        what: what.to_owned(),
        limit: limit as u64,
    }
}

/// Turns raw records (line, cells) into a sheet: the first record with any text is the header row.
fn sheet_of(records: Vec<(u32, Vec<Cell>)>, limits: &Limits) -> Result<Sheet, ImportError> {
    let mut it = records.into_iter().filter(|(_, cells)| {
        cells
            .iter()
            .any(|c| !c.text().trim().is_empty() || matches!(c, Cell::Error(_)))
    });
    let Some((line, header)) = it.next() else {
        return Err(malformed("the table has no header row", None));
    };
    let headers: Vec<String> = header.iter().map(|c| c.text().trim().to_owned()).collect();
    if headers.len() > MAX_COLUMNS {
        return Err(too_many("column count", MAX_COLUMNS));
    }
    if headers.iter().all(String::is_empty) {
        return Err(malformed("the header row is empty", Some(line)));
    }
    let mut rows = Vec::new();
    let mut overhang = 0;
    for (line, mut cells) in it {
        if rows.len() == MAX_ROWS {
            return Err(too_many("row count", MAX_ROWS));
        }
        for c in &cells {
            if c.text().len() > limits.max_text_bytes as usize {
                return Err(ImportError::LimitExceeded {
                    what: format!("cell size at line {line}"),
                    limit: u64::from(limits.max_text_bytes),
                });
            }
        }
        if cells.len() > headers.len() {
            if cells[headers.len()..].iter().any(|c| !c.text().trim().is_empty()) {
                overhang += 1;
            }
            cells.truncate(headers.len());
        }
        cells.resize(headers.len(), Cell::Text(String::new()));
        rows.push(Row { line, cells });
    }
    Ok(Sheet {
        headers,
        rows,
        overhang,
    })
}

// -------------------------------------------------------------------------------------------------------------------
// CSV

/// The separator of the header line: `;` when it has more semicolons than commas outside quotes, else `,`.
fn separator(text: &str) -> char {
    let (mut commas, mut semicolons, mut quoted) = (0usize, 0usize, false);
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => commas += 1,
            ';' if !quoted => semicolons += 1,
            '\n' | '\r' if !quoted => break,
            _ => {}
        }
    }
    if semicolons > commas { ';' } else { ',' }
}

/// Reads CSV text: UTF-8, an optional byte-order mark, `,` or `;`, fields quoted with `"` (a doubled `""` inside a
/// quoted field is one quote; a quoted field may span lines), lines ended by LF or CR LF.
pub(crate) fn read_csv(bytes: &[u8], limits: &Limits) -> Result<Sheet, ImportError> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        let line = bytes[..e.valid_up_to()].iter().filter(|b| **b == b'\n').count() + 1;
        malformed("the table is not UTF-8 text", u32::try_from(line).ok())
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let sep = separator(text);
    let mut records: Vec<(u32, Vec<Cell>)> = Vec::new();
    let mut line: u32 = 1;
    let mut record_line = 1;
    let mut field = String::new();
    let mut cells: Vec<Cell> = Vec::new();
    let mut chars = text.chars().peekable();
    // At the start of a field; inside quotes; just after a closing quote.
    let (mut quoted, mut closed, mut started) = (false, false, false);
    let mut quote_line = 0;
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => {
                    quoted = false;
                    closed = true;
                }
                '\n' => {
                    line += 1;
                    field.push('\n');
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if !started => {
                quoted = true;
                started = true;
                quote_line = line;
            }
            c if c == sep => {
                cells.push(Cell::Text(std::mem::take(&mut field)));
                if cells.len() > MAX_COLUMNS {
                    return Err(too_many("column count", MAX_COLUMNS));
                }
                (closed, started) = (false, false);
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                cells.push(Cell::Text(std::mem::take(&mut field)));
                records.push((record_line, std::mem::take(&mut cells)));
                if records.len() > MAX_ROWS + 1 {
                    return Err(too_many("row count", MAX_ROWS));
                }
                line += 1;
                record_line = line;
                (closed, started) = (false, false);
            }
            _ if closed => {
                return Err(malformed(
                    format!("text after a closing quote: `{c}`"),
                    Some(line),
                ));
            }
            _ => {
                started = true;
                field.push(c);
            }
        }
    }
    if quoted {
        return Err(malformed("a quoted field is not closed", Some(quote_line)));
    }
    if started || !cells.is_empty() {
        cells.push(Cell::Text(field));
        records.push((record_line, cells));
    }
    sheet_of(records, limits)
}

// -------------------------------------------------------------------------------------------------------------------
// XLSX

/// Reads the sheet `sheet` (the first when `None`) of an XLSX workbook ([`crate::xlsx`]: the archive's declared and
/// actual unpacked sizes bounded by `limits.max_input_bytes`, the shared strings by the loader limits), at most
/// [`MAX_COLUMNS`] columns and [`MAX_ROWS`] rows.
pub(crate) fn read_xlsx(bytes: &[u8], sheet: Option<&str>, limits: &Limits) -> Result<Sheet, ImportError> {
    sheet_of(crate::xlsx::read(bytes, sheet, limits)?, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn csv(text: &str) -> Result<Sheet, ImportError> {
        read_csv(text.as_bytes(), &Limits::default())
    }

    fn texts(s: &Sheet) -> Vec<Vec<&str>> {
        s.rows
            .iter()
            .map(|r| r.cells.iter().map(Cell::text).collect())
            .collect()
    }

    #[test]
    fn the_separator_comes_from_the_header_line() {
        let s = csv("a;b,c;d\n1;2,5;3\n").unwrap();
        assert_eq!(s.headers, ["a", "b,c", "d"]);
        assert_eq!(texts(&s), [["1", "2,5", "3"]]);
        let s = csv("\u{feff}a,b\r\n\"x, y\",\"say \"\"hi\"\"\"\r\n").unwrap();
        assert_eq!(s.headers, ["a", "b"]);
        assert_eq!(texts(&s), [["x, y", "say \"hi\""]]);
    }

    #[test]
    fn quoted_fields_span_lines_and_rows_keep_their_line() {
        let s = csv("a,b\n\n1,\"two\nlines\"\n3\n").unwrap();
        assert_eq!(texts(&s), [vec!["1", "two\nlines"], vec!["3", ""]]);
        assert_eq!(s.rows.iter().map(|r| r.line).collect::<Vec<_>>(), [3, 5]);
    }

    #[test]
    fn broken_csv_is_refused_with_its_line() {
        assert_eq!(
            csv("a,b\n1,\"open\n"),
            Err(malformed("a quoted field is not closed", Some(2)))
        );
        assert_eq!(
            csv("a,b\n\"x\"y,1\n"),
            Err(malformed("text after a closing quote: `y`", Some(2)))
        );
        assert_eq!(
            read_csv(b"a,b\n\xff,1\n", &Limits::default()),
            Err(malformed("the table is not UTF-8 text", Some(2)))
        );
        assert_eq!(csv("\n\n"), Err(malformed("the table has no header row", None)));
    }

    #[test]
    fn cells_beyond_the_header_are_counted_not_read() {
        let s = csv("a,b\n1,2,3\n4\n").unwrap();
        assert_eq!(texts(&s), [["1", "2"], ["4", ""]]);
        assert_eq!(s.overhang, 1);
    }

    #[test]
    fn a_cell_above_the_text_limit_refuses_the_table() {
        let limits = Limits {
            max_text_bytes: 4,
            ..Limits::default()
        };
        assert!(matches!(
            read_csv(b"a\n12345\n", &limits),
            Err(ImportError::LimitExceeded { .. })
        ));
    }

    #[test]
    fn a_zip_is_a_workbook() {
        assert_eq!(SourceFormat::detect(b"PK\x03\x04rest"), SourceFormat::Xlsx);
        assert_eq!(SourceFormat::detect(b"a,b\n"), SourceFormat::Csv);
        assert!(matches!(
            read_xlsx(b"PK\x03\x04 not a zip", None, &Limits::default()),
            Err(ImportError::Malformed { .. })
        ));
    }
}
