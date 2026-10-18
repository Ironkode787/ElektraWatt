// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict, bounded CSV reader and writer for the golden CSV dialect.
//!
//! The dialect: UTF-8 without BOM, records end with LF (a CR directly before the LF is dropped), delimiter `,`,
//! RFC 4180 quoting with `"`, first record is the header, no empty lines, every record has the header's field count.

use std::collections::HashSet;
use std::fmt;

/// Bounds of one table file (doc 14 §11: CSV ≤ 1,000,000 rows, ≤ 1,024 columns, cell ≤ 32 KiB).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TableLimits {
    /// Longest accepted input in bytes.
    pub max_file_bytes: u64,
    /// Most accepted data rows (the header does not count).
    pub max_rows: usize,
    /// Most accepted columns.
    pub max_columns: usize,
    /// Longest accepted field in bytes.
    pub max_cell_bytes: usize,
}

impl Default for TableLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_rows: 1_000_000,
            max_columns: 1_024,
            max_cell_bytes: 32 * 1024,
        }
    }
}

/// One parsed table: the header names and the data rows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CsvTable {
    /// Column names, non-empty and unique.
    pub header: Vec<String>,
    /// The data rows in file order.
    pub rows: Vec<CsvRow>,
}

/// One data row: its 1-based line number (the line where the record starts) and its fields, one per header column.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CsvRow {
    /// 1-based line where the record starts.
    pub line: u32,
    /// The fields, verbatim.
    pub cells: Vec<String>,
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

    /// Reads one record starting at `pos` (not at the end of input).
    fn record(&mut self) -> Result<Vec<String>, CsvError> {
        let start_line = line_u32(self.line);
        let b = self.bytes();
        if b[self.pos] == b'\n' || (b[self.pos] == b'\r' && b.get(self.pos + 1) == Some(&b'\n')) {
            return Err(CsvError::EmptyLine { line: start_line });
        }
        let mut fields: Vec<String> = Vec::new();
        loop {
            if fields.len() >= self.limits.max_columns {
                return Err(CsvError::TooManyColumns {
                    line: start_line,
                    limit: self.limits.max_columns,
                });
            }
            let (field, end) = self.field(fields.len() + 1)?;
            fields.push(field);
            if matches!(end, End::Record) {
                return Ok(fields);
            }
        }
    }

    fn too_long(&self, line: usize, column: usize) -> CsvError {
        CsvError::CellTooLong {
            line: line_u32(line),
            column,
            limit: self.limits.max_cell_bytes,
        }
    }

    /// Reads one field and the delimiter or line end after it (consumed).
    fn field(&mut self, column: usize) -> Result<(String, End), CsvError> {
        if self.bytes().get(self.pos) == Some(&b'"') {
            self.quoted(column)
        } else {
            self.unquoted(column)
        }
    }

    fn unquoted(&mut self, column: usize) -> Result<(String, End), CsvError> {
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
            if i - start > self.limits.max_cell_bytes {
                return Err(self.too_long(self.line, column));
            }
        }
        let text = self.s[start..i].to_owned();
        self.pos = i;
        Ok((text, self.end_after_field()))
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

    fn quoted(&mut self, column: usize) -> Result<(String, End), CsvError> {
        let start_line = self.line;
        let max = self.limits.max_cell_bytes;
        let mut out = String::new();
        self.pos += 1; // opening quote
        loop {
            let rest = &self.s[self.pos..];
            let Some(q) = rest.bytes().position(|c| c == b'"') else {
                if out.len() + rest.len() > max {
                    return Err(self.too_long(start_line, column));
                }
                return Err(CsvError::UnterminatedQuote {
                    line: line_u32(start_line),
                });
            };
            let chunk = &rest[..q];
            if out.len() + chunk.len() > max {
                return Err(self.too_long(start_line, column));
            }
            out.push_str(chunk);
            self.line += chunk.bytes().filter(|&c| c == b'\n').count();
            self.pos += q + 1;
            if self.bytes().get(self.pos) == Some(&b'"') {
                if out.len() + 1 > max {
                    return Err(self.too_long(start_line, column));
                }
                out.push('"');
                self.pos += 1;
                continue;
            }
            break;
        }
        if self.at_end() || matches!(self.bytes()[self.pos], b',' | b'\n') {
            return Ok((out, self.end_after_field()));
        }
        if self.bytes()[self.pos] == b'\r' && self.bytes().get(self.pos + 1) == Some(&b'\n') {
            return Ok((out, self.end_after_field()));
        }
        Err(CsvError::StrayQuote {
            line: line_u32(self.line),
        })
    }
}

/// Parses `bytes` as one table of the golden CSV dialect under `limits`. One pass, O(len) time; memory bounded by
/// the output (never allocate more than one cell beyond the cell limit, never more than limit+1 rows/columns).
pub fn parse(bytes: &[u8], limits: &TableLimits) -> Result<CsvTable, CsvError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(CsvError::TooLarge {
            limit: limits.max_file_bytes,
        });
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
    };
    let header = sc.record()?;
    let mut seen = HashSet::new();
    for (i, name) in header.iter().enumerate() {
        if name.is_empty() {
            return Err(CsvError::EmptyColumnName { column: i + 1 });
        }
        if !seen.insert(name.as_str()) {
            return Err(CsvError::DuplicateColumn { name: name.clone() });
        }
    }
    let mut rows: Vec<CsvRow> = Vec::new();
    while !sc.at_end() {
        if rows.len() >= limits.max_rows {
            return Err(CsvError::TooManyRows {
                limit: limits.max_rows,
            });
        }
        let line = line_u32(sc.line);
        let cells = sc.record()?;
        if cells.len() != header.len() {
            return Err(CsvError::RaggedRow {
                line,
                expected: header.len(),
                found: cells.len(),
            });
        }
        rows.push(CsvRow { line, cells });
    }
    Ok(CsvTable { header, rows })
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

    fn p(s: &str) -> Result<CsvTable, CsvError> {
        parse(s.as_bytes(), &TableLimits::default())
    }

    fn lim(rows: usize, cols: usize, cell: usize, file: u64) -> TableLimits {
        TableLimits {
            max_file_bytes: file,
            max_rows: rows,
            max_columns: cols,
            max_cell_bytes: cell,
        }
    }

    fn cells(t: &CsvTable, i: usize) -> Vec<&str> {
        t.rows[i].cells.iter().map(String::as_str).collect()
    }

    #[test]
    fn simple_table() {
        let t = p("a,b\n1,2\n3,\n").unwrap();
        assert_eq!(t.header, ["a", "b"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0].line, 2);
        assert_eq!(cells(&t, 0), ["1", "2"]);
        assert_eq!(t.rows[1].line, 3);
        assert_eq!(cells(&t, 1), ["3", ""]);
    }

    #[test]
    fn quoted_fields() {
        let t = p("a,b\n\"x,y\",\"he said \"\"hi\"\"\"\n\"l1\nl2\",\"c\rd\"\nz,w\n").unwrap();
        assert_eq!(cells(&t, 0), ["x,y", "he said \"hi\""]);
        assert_eq!(cells(&t, 1), ["l1\nl2", "c\rd"]);
        assert_eq!(t.rows[1].line, 3);
        assert_eq!(t.rows[2].line, 5);
    }

    #[test]
    fn crlf_records() {
        let t = p("a,b\r\n1,\"2\"\r\n3,4\r\n").unwrap();
        assert_eq!(cells(&t, 0), ["1", "2"]);
        assert_eq!(cells(&t, 1), ["3", "4"]);
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
        assert_eq!(cells(&t, 0), ["1", "2"]);
        let t = p("a\n\"x\"").unwrap();
        assert_eq!(cells(&t, 0), ["x"]);
        let t = p("a,b\n1,").unwrap();
        assert_eq!(cells(&t, 0), ["1", ""]);
    }

    #[test]
    fn header_only() {
        assert_eq!(p("a,b\n").unwrap().rows.len(), 0);
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
        assert_eq!(cells(&t, 0), fields);
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
