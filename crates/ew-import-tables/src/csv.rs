// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict, bounded CSV reader and writer for the golden CSV dialect.
//!
//! The dialect: UTF-8 without BOM, records end with LF (a CR directly before the LF is dropped), delimiter `,`,
//! RFC 4180 quoting with `"`, first record is the header, no empty lines, every record has the header's field count.
//!
//! # Memory
//!
//! [`parse`] does not allocate per field. A parsed [`CsvTable`] stores each cell as a [`Cell`], a span of 8 bytes
//! into the input: an unquoted field and a quoted field without a doubled `""` are spans of the input itself, and
//! only a quoted field that contains a doubled `""` has its unescaped text appended to one shared side buffer.
//! The table does not own the input; [`CsvTable::cell`] resolves a cell against the same bytes that were parsed.
//! Every cell ends at a delimiter, a line end or the end of the input, so a table has at most one cell per input
//! byte; together with the per-table cell budget ([`TableLimits::max_cells`]) this bounds the table's storage
//! (see [`CsvTable::accounted_bytes`]).

use std::collections::HashSet;
use std::fmt;

/// The largest file a table can be parsed from, whatever [`TableLimits::max_file_bytes`] says: cells address the
/// input and the side buffer with 32-bit offsets, and the side buffer (the unescaped text of quoted fields, and
/// the NFC text of the importer's `Table`) is at most four times the input.
const HARD_MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// The flag in [`Cell`]'s length that marks a cell of the side buffer.
const OWNED: u32 = 1 << 31;

/// Bounds of one table file (doc 14 §11: CSV ≤ 1,000,000 rows, ≤ 1,024 columns, cell ≤ 32 KiB; the cell budget
/// keeps the table's own memory within tens of megabytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TableLimits {
    /// Longest accepted input in bytes (at most 512 MiB, whatever the value).
    pub max_file_bytes: u64,
    /// Most accepted data rows (the header does not count).
    pub max_rows: usize,
    /// Most accepted columns.
    pub max_columns: usize,
    /// Longest accepted field in bytes (at most 2 GiB − 1, whatever the value).
    pub max_cell_bytes: usize,
    /// Most accepted cells of one table, the header cells included. A cell is at least one input byte, so a
    /// file of commas costs at most 8 bytes of table storage per input byte; the default budget caps that at
    /// 32 MB.
    pub max_cells: usize,
}

impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_rows: 1_000_000,
            max_columns: 1_024,
            max_cell_bytes: 32 * 1024,
            max_cells: 4_000_000,
        }
    }
}

/// One cell of a [`CsvTable`]: a span of 8 bytes, either into the input or into the table's side buffer.
///
/// `start` is the byte offset; the top bit of `len` marks the side buffer, the other 31 bits are the length.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Cell {
    start: u32,
    len: u32,
}

/// `n` as `u32`; every caller is bounded by [`HARD_MAX_FILE_BYTES`] (or by the side buffer, at most four times
/// that), so the saturation is never reached.
fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Cell {
    /// A span of the input.
    fn input(start: usize, len: usize) -> Cell {
        Cell {
            start: u32_of(start),
            len: u32_of(len),
        }
    }

    /// A span of the side buffer.
    pub(crate) fn owned(start: usize, len: usize) -> Cell {
        Cell {
            start: u32_of(start),
            len: u32_of(len) | OWNED,
        }
    }

    /// The length of the cell's text in bytes.
    pub fn len(self) -> usize {
        (self.len & !OWNED) as usize
    }

    /// Whether the cell is empty.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// The text of `cell`, resolved against the `input` it was parsed from and the side buffer `owned`. The empty
/// string for a span that is not on a character boundary, which cannot happen for the bytes that were parsed.
pub(crate) fn cell_text<'a>(input: &'a [u8], owned: &'a str, cell: Cell) -> &'a str {
    let start = cell.start as usize;
    let end = start + cell.len();
    if cell.len & OWNED != 0 {
        owned.get(start..end).unwrap_or("")
    } else {
        input
            .get(start..end)
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }
}

/// One parsed table: the header names and the data rows as spans (see the module docs).
///
/// A table does not own the input: resolve cells with [`CsvTable::cell`] and the bytes that were parsed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CsvTable {
    /// Column names, non-empty and unique.
    pub header: Vec<String>,
    /// The number of columns (the header's length).
    pub(crate) ncols: usize,
    /// The 1-based line where each record starts, one per data row.
    pub(crate) lines: Vec<u32>,
    /// The cells of all rows, row-major, `ncols` per row.
    pub(crate) cells: Vec<Cell>,
    /// The unescaped text of quoted fields that contain a doubled `""` (and, in the importer's `Table`, NFC
    /// text).
    pub(crate) owned: String,
}

impl CsvTable {
    /// The number of data rows.
    pub fn row_count(&self) -> usize {
        self.lines.len()
    }

    /// The number of columns.
    pub fn column_count(&self) -> usize {
        self.ncols
    }

    /// The 1-based line where data row `row` starts.
    ///
    /// # Panics
    ///
    /// When `row` is not below [`CsvTable::row_count`].
    pub fn line(&self, row: usize) -> u32 {
        self.lines[row]
    }

    /// The text of cell (`row`, `col`), verbatim; `input` must be the bytes this table was parsed from.
    ///
    /// # Panics
    ///
    /// When `row` or `col` is out of range.
    pub fn cell<'a>(&'a self, input: &'a [u8], row: usize, col: usize) -> &'a str {
        assert!(col < self.ncols, "column {col} out of range");
        cell_text(input, &self.owned, self.cells[row * self.ncols + col])
    }

    /// The bytes this table holds besides the input: the capacities of the cell, line, header and side-buffer
    /// vectors times their element sizes (plus the header names).
    ///
    /// Per-cell storage is at most 12 bytes ([`Cell`] is 8) and every cell costs at least one input byte (it
    /// ends at a delimiter, a line end or the end of the input), so the cell storage is at most 12 times the
    /// input; the side buffer holds unescaped text, at most the input again; the header costs at most the
    /// input plus 24 bytes per column. The figure does not include the input itself.
    pub fn accounted_bytes(&self) -> usize {
        self.header.capacity() * size_of::<String>()
            + self.header.iter().map(String::capacity).sum::<usize>()
            + self.lines.capacity() * size_of::<u32>()
            + self.cells.capacity() * size_of::<Cell>()
            + self.owned.capacity()
    }
}

/// Why a file is not a table of the dialect. Every variant that has a line carries the 1-based line of the problem.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CsvError {
    /// The input is longer than `max_file_bytes`.
    TooLarge {
        /// The configured limit.
        limit: u64,
    },
    /// More than `max_rows` data rows.
    TooManyRows {
        /// The configured limit.
        limit: usize,
    },
    /// More than `max_cells` cells (the header's included).
    TooManyCells {
        /// The configured limit.
        limit: usize,
    },
    /// A record with more than `max_columns` fields.
    TooManyColumns {
        /// Line where the record starts.
        line: u32,
        /// The configured limit.
        limit: usize,
    },
    /// A field longer than `max_cell_bytes` bytes.
    CellTooLong {
        /// Line where the field starts.
        line: u32,
        /// 1-based column.
        column: usize,
        /// The configured limit.
        limit: usize,
    },
    /// The input starts with a UTF-8 byte order mark.
    Bom,
    /// The input is not valid UTF-8.
    NotUtf8 {
        /// Line of the first invalid byte.
        line: u32,
    },
    /// There is no header record.
    Empty,
    /// A header name is empty.
    EmptyColumnName {
        /// 1-based column.
        column: usize,
    },
    /// Two header names are equal.
    DuplicateColumn {
        /// The repeated name.
        name: String,
    },
    /// An empty line.
    EmptyLine {
        /// The line.
        line: u32,
    },
    /// A quote inside an unquoted field, or text after a closing quote.
    StrayQuote {
        /// The line.
        line: u32,
    },
    /// A CR that is not part of a CRLF line end and not inside quotes.
    StrayCr {
        /// The line.
        line: u32,
    },
    /// A quoted field that is never closed.
    UnterminatedQuote {
        /// Line where the quoted field started.
        line: u32,
    },
    /// A record whose field count differs from the header's.
    RaggedRow {
        /// Line where the record starts.
        line: u32,
        /// The header's field count.
        expected: usize,
        /// The record's field count.
        found: usize,
    },
}

impl fmt::Display for CsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "file is larger than {limit} bytes"),
            Self::TooManyRows { limit } => write!(f, "more than {limit} data rows"),
            Self::TooManyCells { limit } => write!(f, "more than {limit} cells"),
            Self::TooManyColumns { line, limit } => {
                write!(f, "line {line}: more than {limit} columns")
            }
            Self::CellTooLong { line, column, limit } => {
                write!(f, "line {line}, column {column}: cell longer than {limit} bytes")
            }
            Self::Bom => write!(f, "file starts with a byte order mark"),
            Self::NotUtf8 { line } => write!(f, "line {line}: not valid UTF-8"),
            Self::Empty => write!(f, "file has no header record"),
            Self::EmptyColumnName { column } => write!(f, "column {column} has an empty name"),
            Self::DuplicateColumn { name } => write!(f, "duplicate column name {name:?}"),
            Self::EmptyLine { line } => write!(f, "line {line}: empty line"),
            Self::StrayQuote { line } => write!(f, "line {line}: stray quote"),
            Self::StrayCr { line } => write!(f, "line {line}: stray carriage return"),
            Self::UnterminatedQuote { line } => {
                write!(f, "line {line}: quoted field is not terminated")
            }
            Self::RaggedRow {
                line,
                expected,
                found,
            } => write!(f, "line {line}: expected {expected} fields, found {found}"),
        }
    }
}

impl std::error::Error for CsvError {}

fn line_u32(line: usize) -> u32 {
    u32::try_from(line).unwrap_or(u32::MAX)
}

/// How a field ended.
enum End {
    Comma,
    Record,
}

struct Scanner<'a> {
    s: &'a str,
    pos: usize,
    /// 1-based current line.
    line: usize,
    limits: &'a TableLimits,
    /// The effective cell-length limit (`max_cell_bytes`, within the 31 bits of a cell).
    max_cell: usize,
    /// The cells read since the last [`Scanner::reset`].
    cells: Vec<Cell>,
    /// The side buffer: unescaped text of quoted fields with a doubled quote.
    owned: String,
    /// All cells read so far, the header's included (the cell budget).
    counted: usize,
}

impl Scanner<'_> {
    fn bytes(&self) -> &[u8] {
        self.s.as_bytes()
    }

    fn at_end(&self) -> bool {
        self.pos >= self.s.len()
    }

    /// Consumes a line end (LF or CRLF) at `pos`; `false` when none is there.
    fn eat_line_end(&mut self) -> bool {
        let b = self.bytes();
        if b.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
            self.line += 1;
            true
        } else if b.get(self.pos) == Some(&b'\r') && b.get(self.pos + 1) == Some(&b'\n') {
            self.pos += 2;
            self.line += 1;
            true
        } else {
            false
        }
    }

    /// Reads one record starting at `pos` (not at the end of input), appending its cells to `self.cells`;
    /// returns the field count.
    fn record(&mut self) -> Result<usize, CsvError> {
        let start_line = line_u32(self.line);
        let b = self.bytes();
        if b[self.pos] == b'\n' || (b[self.pos] == b'\r' && b.get(self.pos + 1) == Some(&b'\n')) {
            return Err(CsvError::EmptyLine { line: start_line });
        }
        let mut fields = 0usize;
        loop {
            if fields >= self.limits.max_columns {
                return Err(CsvError::TooManyColumns {
                    line: start_line,
                    limit: self.limits.max_columns,
                });
            }
            if self.counted >= self.limits.max_cells {
                return Err(CsvError::TooManyCells {
                    limit: self.limits.max_cells,
                });
            }
            let (cell, end) = self.field(fields + 1)?;
            self.counted += 1;
            self.cells.push(cell);
            fields += 1;
            if matches!(end, End::Record) {
                return Ok(fields);
            }
        }
    }

    /// Forgets the cells and the side buffer (the header's, once its names are copied out).
    fn reset(&mut self) {
        self.cells.clear();
        self.owned.clear();
    }

    fn too_long(&self, line: usize, column: usize) -> CsvError {
        CsvError::CellTooLong {
            line: line_u32(line),
            column,
            limit: self.limits.max_cell_bytes,
        }
    }

    /// Reads one field and the delimiter or line end after it (consumed).
    fn field(&mut self, column: usize) -> Result<(Cell, End), CsvError> {
        if self.bytes().get(self.pos) == Some(&b'"') {
            self.quoted(column)
        } else {
            self.unquoted(column)
        }
    }

    fn unquoted(&mut self, column: usize) -> Result<(Cell, End), CsvError> {
        let start = self.pos;
        let b = self.s.as_bytes();
        let mut i = start;
        while i < b.len() {
            match b[i] {
                b',' | b'\n' => break,
                b'"' => {
                    self.pos = i;
                    return Err(CsvError::StrayQuote {
                        line: line_u32(self.line),
                    });
                }
                b'\r' => {
                    if b.get(i + 1) == Some(&b'\n') {
                        break;
                    }
                    self.pos = i;
                    return Err(CsvError::StrayCr {
                        line: line_u32(self.line),
                    });
                }
                _ => {}
            }
            i += 1;
            if i - start > self.max_cell {
                return Err(self.too_long(self.line, column));
            }
        }
        self.pos = i;
        Ok((Cell::input(start, i - start), self.end_after_field()))
    }

    /// Consumes the delimiter or line end at `pos` (or nothing at end of input).
    fn end_after_field(&mut self) -> End {
        if self.at_end() {
            return End::Record;
        }
        if self.bytes()[self.pos] == b',' {
            self.pos += 1;
            return End::Comma;
        }
        self.eat_line_end();
        End::Record
    }

    /// A quoted field. Its text is a span of the input unless it contains a doubled quote, in which case the
    /// unescaped text is appended to the side buffer.
    fn quoted(&mut self, column: usize) -> Result<(Cell, End), CsvError> {
        let start_line = self.line;
        let max = self.max_cell;
        self.pos += 1; // opening quote
        let content_start = self.pos;
        let owned_start = self.owned.len();
        // Whether the text is assembled in the side buffer, and where the pending literal run starts.
        let mut escaped = false;
        let mut seg = content_start;
        let mut len = 0usize;
        let close;
        loop {
            let rest = &self.s[self.pos..];
            let Some(q) = rest.bytes().position(|c| c == b'"') else {
                if len + rest.len() > max {
                    return Err(self.too_long(start_line, column));
                }
                return Err(CsvError::UnterminatedQuote {
                    line: line_u32(start_line),
                });
            };
            if len + q > max {
                return Err(self.too_long(start_line, column));
            }
            len += q;
            self.line += rest.as_bytes()[..q].iter().filter(|&&c| c == b'\n').count();
            let quote_at = self.pos + q;
            self.pos = quote_at + 1;
            if self.bytes().get(self.pos) == Some(&b'"') {
                if len + 1 > max {
                    return Err(self.too_long(start_line, column));
                }
                len += 1;
                // The literal run up to and including the first quote of the pair; the second is skipped.
                escaped = true;
                self.owned.push_str(&self.s[seg..=quote_at]);
                self.pos += 1;
                seg = self.pos;
                continue;
            }
            close = quote_at;
            break;
        }
        let cell = if escaped {
            self.owned.push_str(&self.s[seg..close]);
            Cell::owned(owned_start, len)
        } else {
            Cell::input(content_start, len)
        };
        if self.at_end() || matches!(self.bytes()[self.pos], b',' | b'\n') {
            return Ok((cell, self.end_after_field()));
        }
        if self.bytes()[self.pos] == b'\r' && self.bytes().get(self.pos + 1) == Some(&b'\n') {
            return Ok((cell, self.end_after_field()));
        }
        Err(CsvError::StrayQuote {
            line: line_u32(self.line),
        })
    }
}

/// Parses `bytes` as one table of the golden CSV dialect under `limits`. One pass, O(len) time; no allocation
/// per field: cells are 8-byte spans into `bytes` (plus one shared side buffer for the unescaped text of
/// quoted fields with a doubled quote), at most one cell per input byte and never more than
/// [`TableLimits::max_cells`]; the vectors are shrunk to fit at the end. Resolve cells with [`CsvTable::cell`]
/// and the same `bytes`.
pub fn parse(bytes: &[u8], limits: &TableLimits) -> Result<CsvTable, CsvError> {
    let max_file = limits.max_file_bytes.min(HARD_MAX_FILE_BYTES);
    if bytes.len() as u64 > max_file {
        return Err(CsvError::TooLarge { limit: max_file });
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(CsvError::Bom);
    }
    let text = std::str::from_utf8(bytes).map_err(|e| CsvError::NotUtf8 {
        line: line_u32(bytes[..e.valid_up_to()].iter().filter(|&&c| c == b'\n').count() + 1),
    })?;
    if text.is_empty() {
        return Err(CsvError::Empty);
    }
    let mut sc = Scanner {
        s: text,
        pos: 0,
        line: 1,
        limits,
        max_cell: limits.max_cell_bytes.min((OWNED - 1) as usize),
        cells: Vec::new(),
        owned: String::new(),
        counted: 0,
    };
    sc.record()?;
    let header: Vec<String> = sc
        .cells
        .iter()
        .map(|&c| cell_text(bytes, &sc.owned, c).to_owned())
        .collect();
    let mut seen = HashSet::new();
    for (i, name) in header.iter().enumerate() {
        if name.is_empty() {
            return Err(CsvError::EmptyColumnName { column: i + 1 });
        }
        if !seen.insert(name.as_str()) {
            return Err(CsvError::DuplicateColumn { name: name.clone() });
        }
    }
    sc.reset();
    let mut lines: Vec<u32> = Vec::new();
    while !sc.at_end() {
        if lines.len() >= limits.max_rows {
            return Err(CsvError::TooManyRows {
                limit: limits.max_rows,
            });
        }
        let line = line_u32(sc.line);
        let found = sc.record()?;
        if found != header.len() {
            return Err(CsvError::RaggedRow {
                line,
                expected: header.len(),
                found,
            });
        }
        lines.push(line);
    }
    let Scanner {
        mut cells, mut owned, ..
    } = sc;
    cells.shrink_to_fit();
    lines.shrink_to_fit();
    owned.shrink_to_fit();
    Ok(CsvTable {
        ncols: header.len(),
        header,
        lines,
        cells,
        owned,
    })
}

/// `field` quoted for the dialect: wrapped in `"` (inner `"` doubled) exactly when it contains `,`, `"`, LF or CR,
/// or begins or ends with a space; otherwise unchanged.
pub fn quote_field(field: &str) -> String {
    let needs = field.contains([',', '"', '\n', '\r']) || field.starts_with(' ') || field.ends_with(' ');
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

/// One record line: the quoted fields joined by `,`, followed by LF.
pub fn write_record<S: AsRef<str>>(fields: &[S]) -> String {
    let mut out = fields
        .iter()
        .map(|f| quote_field(f.as_ref()))
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A parsed table with the text it was parsed from.
    #[derive(Debug, PartialEq)]
    struct P {
        text: String,
        t: CsvTable,
    }

    impl std::ops::Deref for P {
        type Target = CsvTable;
        fn deref(&self) -> &CsvTable {
            &self.t
        }
    }

    impl P {
        fn row(&self, i: usize) -> Vec<&str> {
            (0..self.t.column_count())
                .map(|c| self.t.cell(self.text.as_bytes(), i, c))
                .collect()
        }
    }

    fn p(s: &str) -> Result<P, CsvError> {
        parse(s.as_bytes(), &TableLimits::default()).map(|t| P {
            text: s.to_owned(),
            t,
        })
    }

    fn lim(rows: usize, cols: usize, cell: usize, file: u64) -> TableLimits {
        TableLimits {
            max_file_bytes: file,
            max_rows: rows,
            max_columns: cols,
            max_cell_bytes: cell,
            max_cells: usize::MAX,
        }
    }

    #[test]
    fn a_cell_is_at_most_12_bytes() {
        assert!(size_of::<Cell>() <= 12, "{} bytes", size_of::<Cell>());
    }

    #[test]
    fn cell_budget_counts_header_cells() {
        let limits = |max_cells| TableLimits {
            max_cells,
            ..TableLimits::default()
        };
        // 2 header cells + 2 rows of 2 cells = 6.
        let text = b"a,b\n1,2\n3,4\n";
        assert!(parse(text, &limits(6)).is_ok());
        assert_eq!(parse(text, &limits(5)), Err(CsvError::TooManyCells { limit: 5 }));
        assert_eq!(parse(text, &limits(1)), Err(CsvError::TooManyCells { limit: 1 }));
    }

    #[test]
    fn only_escaped_quotes_use_the_side_buffer() {
        let t = p("a,b,c\nplain,\"quoted, but plain\",\"he said \"\"hi\"\"\"\n").unwrap();
        assert_eq!(t.row(0), ["plain", "quoted, but plain", "he said \"hi\""]);
        assert_eq!(t.cells[0].len & OWNED, 0);
        assert_eq!(t.cells[1].len & OWNED, 0);
        assert_ne!(t.cells[2].len & OWNED, 0);
        assert_eq!(t.owned, "he said \"hi\"");
    }

    #[test]
    fn escaped_text_in_the_middle_and_at_the_ends() {
        let t = p("a\n\"\"\"x\"\"y\"\"\"\n\"\"\"\"\n\"a\"\"\"\n").unwrap();
        assert_eq!(t.row(0), ["\"x\"y\""]);
        assert_eq!(t.row(1), ["\""]);
        assert_eq!(t.row(2), ["a\""]);
    }

    #[test]
    fn simple_table() {
        let t = p("a,b\n1,2\n3,\n").unwrap();
        assert_eq!(t.header, ["a", "b"]);
        assert_eq!(t.row_count(), 2);
        assert_eq!(t.line(0), 2);
        assert_eq!(t.row(0), ["1", "2"]);
        assert_eq!(t.line(1), 3);
        assert_eq!(t.row(1), ["3", ""]);
    }

    #[test]
    fn quoted_fields() {
        let t = p("a,b\n\"x,y\",\"he said \"\"hi\"\"\"\n\"l1\nl2\",\"c\rd\"\nz,w\n").unwrap();
        assert_eq!(t.row(0), ["x,y", "he said \"hi\""]);
        assert_eq!(t.row(1), ["l1\nl2", "c\rd"]);
        assert_eq!(t.line(1), 3);
        assert_eq!(t.line(2), 5);
    }

    #[test]
    fn crlf_records() {
        let t = p("a,b\r\n1,\"2\"\r\n3,4\r\n").unwrap();
        assert_eq!(t.row(0), ["1", "2"]);
        assert_eq!(t.row(1), ["3", "4"]);
        assert_eq!(
            p("a,b\r\n1,2\r\n\"q\"\r\n"),
            Err(CsvError::RaggedRow {
                line: 3,
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn missing_final_lf() {
        let t = p("a,b\n1,2").unwrap();
        assert_eq!(t.row(0), ["1", "2"]);
        let t = p("a\n\"x\"").unwrap();
        assert_eq!(t.row(0), ["x"]);
        let t = p("a,b\n1,").unwrap();
        assert_eq!(t.row(0), ["1", ""]);
    }

    #[test]
    fn header_only() {
        assert_eq!(p("a,b\n").unwrap().row_count(), 0);
        assert_eq!(p("a,b").unwrap().header, ["a", "b"]);
    }

    #[test]
    fn err_too_large() {
        assert_eq!(
            parse(b"a\n1\n", &lim(10, 10, 10, 3)),
            Err(CsvError::TooLarge { limit: 3 })
        );
    }

    #[test]
    fn err_too_many_rows() {
        assert_eq!(
            parse(b"a\n1\n2\n3\n", &lim(2, 10, 10, 100)),
            Err(CsvError::TooManyRows { limit: 2 })
        );
    }

    #[test]
    fn err_too_many_columns() {
        assert_eq!(
            parse(b"a,b\n1,2,3\n", &lim(10, 2, 10, 100)),
            Err(CsvError::TooManyColumns { line: 2, limit: 2 })
        );
    }

    #[test]
    fn err_cell_too_long() {
        assert_eq!(
            parse(b"a,b\n1,123456\n", &lim(10, 10, 5, 100)),
            Err(CsvError::CellTooLong {
                line: 2,
                column: 2,
                limit: 5
            })
        );
        assert_eq!(
            parse(b"a,b\n1,\"12\n3456\"\n", &lim(10, 10, 5, 100)),
            Err(CsvError::CellTooLong {
                line: 2,
                column: 2,
                limit: 5
            })
        );
    }

    #[test]
    fn err_bom() {
        assert_eq!(p("\u{feff}a\n"), Err(CsvError::Bom));
    }

    #[test]
    fn err_not_utf8() {
        assert_eq!(
            parse(b"a\nb\n\xff\n", &TableLimits::default()),
            Err(CsvError::NotUtf8 { line: 3 })
        );
    }

    #[test]
    fn err_empty() {
        assert_eq!(p(""), Err(CsvError::Empty));
    }

    #[test]
    fn err_empty_column_name() {
        assert_eq!(p("a,,c\n"), Err(CsvError::EmptyColumnName { column: 2 }));
    }

    #[test]
    fn err_duplicate_column() {
        assert_eq!(p("a,b,a\n"), Err(CsvError::DuplicateColumn { name: "a".into() }));
    }

    #[test]
    fn err_empty_line() {
        assert_eq!(p("a\n1\n\n2\n"), Err(CsvError::EmptyLine { line: 3 }));
        assert_eq!(p("a\n1\n\n"), Err(CsvError::EmptyLine { line: 3 }));
        assert_eq!(p("\na\n"), Err(CsvError::EmptyLine { line: 1 }));
        assert_eq!(p("a\r\n1\r\n\r\n"), Err(CsvError::EmptyLine { line: 3 }));
    }

    #[test]
    fn err_stray_quote() {
        assert_eq!(p("a\nx\"y\n"), Err(CsvError::StrayQuote { line: 2 }));
        assert_eq!(p("a\n\"x\"y\n"), Err(CsvError::StrayQuote { line: 2 }));
        assert_eq!(p("a,b\n\"x\ny\"z,1\n"), Err(CsvError::StrayQuote { line: 3 }));
    }

    #[test]
    fn err_stray_cr() {
        assert_eq!(p("a\nx\ry\n"), Err(CsvError::StrayCr { line: 2 }));
        assert_eq!(p("a\n1\r"), Err(CsvError::StrayCr { line: 2 }));
    }

    #[test]
    fn err_unterminated_quote() {
        assert_eq!(
            p("a\n1\n\"abc\ndef\n"),
            Err(CsvError::UnterminatedQuote { line: 3 })
        );
    }

    #[test]
    fn err_ragged_row() {
        assert_eq!(
            p("a,b\n1,2\n1,2,3\n"),
            Err(CsvError::RaggedRow {
                line: 3,
                expected: 2,
                found: 3
            })
        );
        assert_eq!(
            p("a,b\n1\n"),
            Err(CsvError::RaggedRow {
                line: 2,
                expected: 2,
                found: 1
            })
        );
    }

    #[test]
    fn limits_at_and_over() {
        // rows
        assert!(parse(b"a\n1\n2\n", &lim(2, 5, 5, 100)).is_ok());
        assert!(parse(b"a\n1\n2\n3\n", &lim(2, 5, 5, 100)).is_err());
        // columns
        assert!(parse(b"a,b\n1,2\n", &lim(5, 2, 5, 100)).is_ok());
        assert!(parse(b"a,b,c\n1,2,3\n", &lim(5, 2, 5, 100)).is_err());
        // cell bytes (U+2212 is 3 bytes)
        assert!(parse("a\n12345\n".as_bytes(), &lim(5, 5, 5, 100)).is_ok());
        assert!(parse("a\n123456\n".as_bytes(), &lim(5, 5, 5, 100)).is_err());
        assert!(parse("a\n\"1234\"\"\"\n".as_bytes(), &lim(5, 5, 5, 100)).is_ok());
        assert!(parse("a\n\"12345\"\"\"\n".as_bytes(), &lim(5, 5, 5, 100)).is_err());
        assert!(parse("a\n\u{2212}x\n".as_bytes(), &lim(5, 5, 4, 100)).is_ok());
        assert!(parse("a\n\u{2212}xy\n".as_bytes(), &lim(5, 5, 4, 100)).is_err());
        // file bytes
        assert!(parse(b"a\n1\n", &lim(5, 5, 5, 4)).is_ok());
        assert!(parse(b"a\n1\n", &lim(5, 5, 5, 3)).is_err());
    }

    #[test]
    fn huge_unterminated_quote_is_refused() {
        let mut v = vec![b'"'];
        v.extend(std::iter::repeat_n(b'a', 4 * 1024 * 1024));
        assert_eq!(
            parse(&v, &TableLimits::default()),
            Err(CsvError::CellTooLong {
                line: 1,
                column: 1,
                limit: 32 * 1024
            })
        );
    }

    #[test]
    fn round_trip() {
        let fields = [
            "plain",
            "a,b",
            "say \"x\"",
            "two\nlines",
            "cr\rhere",
            " lead",
            "trail ",
            "\u{2212}5",
            "",
            "\"",
        ];
        let mut text = write_record(&["h0", "h1", "h2", "h3", "h4", "h5", "h6", "h7", "h8", "h9"]);
        text.push_str(&write_record(&fields));
        let t = p(&text).unwrap();
        assert_eq!(t.row(0), fields);
    }

    #[test]
    fn quote_field_cases() {
        assert_eq!(quote_field("plain"), "plain");
        assert_eq!(quote_field(" x"), "\" x\"");
        assert_eq!(quote_field("x "), "\"x \"");
        assert_eq!(quote_field("a b"), "a b");
        assert_eq!(quote_field("\u{2212}1"), "\u{2212}1");
        assert_eq!(quote_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(quote_field("a,b"), "\"a,b\"");
        assert_eq!(quote_field(""), "");
        assert_eq!(write_record(&["a", "b,c"]), "a,\"b,c\"\n");
    }
}
