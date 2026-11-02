// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CSV text of a report block's rows (doc 09 §1.3; `ew export cables|bom`, the golden `expected/csv/`).
//!
//! One writer for every consumer: the CLI exports, the acceptance runner and the golden files are all this
//! function over [`RowSet`]. The dialect is the one of the golden README ("CSV dialect"): UTF-8, LF line ends,
//! a header line, a field quoted exactly when it contains `,`, `"`, LF or CR or begins or ends with a space,
//! quotes doubled. The first column is `row`: `data` for a data row and `heading` for every other kind; the
//! other columns are the block's columns, headed by their neutral labels and holding the printed cells
//! ([`crate::render::render`] in neutral language).
//!
//! **The presets** (WP-S3.08 D6; [`table_csv`] over a [`Table`]): `excel-nl` (`;`, a decimal comma in number cells,
//! a UTF-8 byte-order mark, CR LF), `excel-en` (`,`, a decimal point, a byte-order mark, CR LF) and `rfc4180` (`,`,
//! CR LF, no byte-order mark, values verbatim). They write the `id` column first (a CSV has no hidden column; a
//! heading row's is empty) and no `row` column, and quote a field exactly when it holds the delimiter, `"`, CR or LF
//! (RFC 4180), quotes doubled. **The guard** (NFR-SEC-17, doc 09 OQ 8): in `excel-nl` and `excel-en` a text cell (and a
//! header) whose first character is `=`, `+`, `-`, `@`, tab or CR gets a leading apostrophe; a number cell never does;
//! `ew` and `rfc4180` never alter a value. [`CsvOptions`] overrides the preset's byte-order mark and line end
//! (FR-EXP-003); the encoding is UTF-8 only. The golden dialect stays the default `ew` ([`Preset::Ew`]): [`rows_csv`],
//! byte for byte, which [`restyle`] gives another byte-order mark or line end.

use ew_derive::Q;

use crate::render::{RenderOptions, render};
use crate::rowset::{RowKind, RowSet};
use crate::table::{Table, TableCell};

/// One field of the golden CSV dialect.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// The printed cells of row `i` of `set` (neutral language, no page).
pub fn row_texts(q: &Q, set: &RowSet, i: usize) -> Vec<String> {
    set.rows[i]
        .cells
        .iter()
        .map(|c| render(q, c, &RenderOptions::default()))
        .collect()
}

/// The printed rows of a block as CSV: the header (`row` and the column labels), then `data` or `heading` and
/// the printed cells of every row.
pub fn rows_csv(q: &Q, set: &RowSet) -> String {
    let mut out = String::new();
    let header: Vec<String> = std::iter::once("row".to_owned())
        .chain(
            set.columns
                .iter()
                .map(|c| csv_field(c.label.as_neutral().unwrap_or(&c.key))),
        )
        .collect();
    out.push_str(&header.join(","));
    out.push('\n');
    for i in 0..set.rows.len() {
        let kind = if set.rows[i].kind == RowKind::Data {
            "data"
        } else {
            "heading"
        };
        let cells: Vec<String> = std::iter::once(kind.to_owned())
            .chain(row_texts(q, set, i).iter().map(|t| csv_field(t)))
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

/// A CSV dialect of the row exports (WP-S3.08 D6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    /// The golden dialect (the default): [`rows_csv`].
    Ew,
    /// `;`, a decimal comma, a byte-order mark, CR LF, the guard.
    ExcelNl,
    /// `,`, a decimal point, a byte-order mark, CR LF, the guard.
    ExcelEn,
    /// `,`, CR LF, no byte-order mark, values verbatim.
    Rfc4180,
}

impl Preset {
    /// Every preset, in the order the refusal names them.
    pub const ALL: [Preset; 4] = [Preset::Ew, Preset::ExcelNl, Preset::ExcelEn, Preset::Rfc4180];

    /// The preset's name (`--preset <name>`).
    pub fn name(self) -> &'static str {
        match self {
            Preset::Ew => "ew",
            Preset::ExcelNl => "excel-nl",
            Preset::ExcelEn => "excel-en",
            Preset::Rfc4180 => "rfc4180",
        }
    }

    /// The preset named `name`.
    pub fn parse(name: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.name() == name)
    }

    /// The field delimiter.
    fn delimiter(self) -> char {
        if self == Preset::ExcelNl { ';' } else { ',' }
    }

    /// Whether the preset writes a byte-order mark.
    fn bom(self) -> bool {
        matches!(self, Preset::ExcelNl | Preset::ExcelEn)
    }

    /// The preset's line end.
    fn line_end(self) -> LineEnd {
        if self == Preset::Ew {
            LineEnd::Lf
        } else {
            LineEnd::Crlf
        }
    }

    /// Whether the preset guards text cells against formula injection.
    fn guards(self) -> bool {
        matches!(self, Preset::ExcelNl | Preset::ExcelEn)
    }
}

/// The line end of a CSV.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineEnd {
    /// LF.
    Lf,
    /// CR LF.
    Crlf,
}

impl LineEnd {
    /// The name (`--line-end <name>`).
    pub fn name(self) -> &'static str {
        match self {
            LineEnd::Lf => "lf",
            LineEnd::Crlf => "crlf",
        }
    }

    /// The line end named `name`.
    pub fn parse(name: &str) -> Option<LineEnd> {
        [LineEnd::Crlf, LineEnd::Lf]
            .into_iter()
            .find(|l| l.name() == name)
    }

    fn text(self) -> &'static str {
        match self {
            LineEnd::Lf => "\n",
            LineEnd::Crlf => "\r\n",
        }
    }
}

/// How a CSV is written: a preset, and the byte-order mark and line end that override it (`--bom`, `--line-end`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CsvOptions {
    /// The preset.
    pub preset: Preset,
    /// The byte-order mark (`--bom on|off`); `None`: the preset's.
    pub bom: Option<bool>,
    /// The line end (`--line-end crlf|lf`); `None`: the preset's.
    pub line_end: Option<LineEnd>,
}

impl CsvOptions {
    /// The options of `preset` alone.
    pub fn preset(preset: Preset) -> CsvOptions {
        CsvOptions {
            preset,
            bom: None,
            line_end: None,
        }
    }

    /// Whether the CSV starts with a byte-order mark.
    pub fn writes_bom(&self) -> bool {
        self.bom.unwrap_or_else(|| self.preset.bom())
    }

    /// The CSV's line end.
    pub fn writes_line_end(&self) -> LineEnd {
        self.line_end.unwrap_or_else(|| self.preset.line_end())
    }
}

impl Default for CsvOptions {
    fn default() -> CsvOptions {
        CsvOptions::preset(Preset::Ew)
    }
}

/// `csv`, a text in the golden dialect (LF record ends, no byte-order mark), with a byte-order mark when `bom` and its
/// record ends written as `line_end`; a line break inside a quoted field is kept as it is.
pub fn restyle(csv: &str, bom: bool, line_end: LineEnd) -> String {
    let mut out = String::with_capacity(csv.len() + csv.len() / 16 + 3);
    if bom {
        out.push('\u{feff}');
    }
    let mut quoted = false;
    for c in csv.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                out.push(c);
            }
            '\n' if !quoted => out.push_str(line_end.text()),
            c => out.push(c),
        }
    }
    out
}

/// The CSV of `table` in `options`' preset (the module documentation): for [`Preset::Ew`] the golden layout (`row`
/// first, `data` or `heading`, the cells' texts), for the others the `id` column first.
pub fn table_csv(table: &Table, options: &CsvOptions) -> String {
    let preset = options.preset;
    let end = options.writes_line_end().text();
    let mut out = String::new();
    if options.writes_bom() {
        out.push('\u{feff}');
    }
    let field = |text: &str, guard: bool| -> String {
        if preset == Preset::Ew {
            return csv_field(text);
        }
        if guard && preset.guards() && text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            quoted(&format!("'{text}"), preset.delimiter())
        } else {
            quoted(text, preset.delimiter())
        }
    };
    let delimiter = preset.delimiter().to_string();
    let first = if preset == Preset::Ew { "row" } else { "id" };
    let header: Vec<String> = std::iter::once(first.to_owned())
        .chain(table.columns.iter().map(|c| field(&c.label, true)))
        .collect();
    out.push_str(&header.join(&delimiter));
    out.push_str(end);
    for row in &table.rows {
        let lead = match (preset, row.heading) {
            (Preset::Ew, true) => "heading".to_owned(),
            (Preset::Ew, false) => "data".to_owned(),
            _ => field(row.id.as_deref().unwrap_or_default(), false),
        };
        let cells: Vec<String> = std::iter::once(lead)
            .chain(row.cells.iter().map(|c| match c {
                TableCell::Number(n) if preset == Preset::ExcelNl => field(&n.replace('.', ","), false),
                TableCell::Number(n) | TableCell::Date(n) => field(n, false),
                TableCell::Text(t) => field(t, true),
            }))
            .collect();
        out.push_str(&cells.join(&delimiter));
        out.push_str(end);
    }
    out
}

/// A field of a preset: quoted when it holds the delimiter, `"`, CR or LF (RFC 4180), quotes doubled.
fn quoted(text: &str, delimiter: char) -> String {
    if text.contains([delimiter, '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rowset::CellType;
    use crate::table::{TableColumn, TableRow};

    fn table(cells: Vec<TableCell>) -> Table {
        Table {
            name: "t".into(),
            key: "sql".into(),
            columns: cells
                .iter()
                .enumerate()
                .map(|(i, _)| TableColumn {
                    key: format!("c{i}"),
                    label: format!("C{i}"),
                    cell: CellType::Text,
                    editable: false,
                })
                .collect(),
            rows: vec![
                TableRow {
                    id: Some("e:1".into()),
                    heading: false,
                    cells,
                },
                TableRow {
                    id: None,
                    heading: true,
                    cells: vec![TableCell::Text("-X1".into())],
                },
            ],
        }
    }

    /// TX-4 (NFR-SEC-17): the Excel presets guard a text cell that starts like a formula, quoted as RFC 4180 asks;
    /// `rfc4180` writes it verbatim; a number cell is never guarded.
    #[test]
    fn the_excel_presets_guard_text_and_never_numbers() {
        let t = table(vec![
            TableCell::Text("=HYPERLINK(\"x\")".into()),
            TableCell::Text("-3X1".into()),
            TableCell::Number("-1.5".into()),
        ]);
        assert_eq!(
            table_csv(&t, &CsvOptions::preset(Preset::ExcelNl)),
            "\u{feff}id;C0;C1;C2\r\ne:1;\"'=HYPERLINK(\"\"x\"\")\";'-3X1;-1,5\r\n;'-X1\r\n"
        );
        assert_eq!(
            table_csv(&t, &CsvOptions::preset(Preset::ExcelEn)),
            "\u{feff}id,C0,C1,C2\r\ne:1,\"'=HYPERLINK(\"\"x\"\")\",'-3X1,-1.5\r\n,'-X1\r\n"
        );
        assert_eq!(
            table_csv(&t, &CsvOptions::preset(Preset::Rfc4180)),
            "id,C0,C1,C2\r\ne:1,\"=HYPERLINK(\"\"x\"\")\",-3X1,-1.5\r\n,-X1\r\n"
        );
    }

    /// `--bom` and `--line-end` override the preset; `ew` is the golden layout.
    #[test]
    fn options_override_the_preset() {
        let t = table(vec![TableCell::Text("a;b".into())]);
        let options = CsvOptions {
            preset: Preset::ExcelNl,
            bom: Some(false),
            line_end: Some(LineEnd::Lf),
        };
        assert_eq!(table_csv(&t, &options), "id;C0\ne:1;\"a;b\"\n;'-X1\n");
        assert_eq!(
            table_csv(&t, &CsvOptions::default()),
            "row,C0\ndata,a;b\nheading,-X1\n"
        );
    }

    #[test]
    fn restyle_changes_record_ends_only() {
        let csv = "a,\"two\nlines\"\nb,c\n";
        assert_eq!(restyle(csv, false, LineEnd::Lf), csv);
        assert_eq!(
            restyle(csv, true, LineEnd::Crlf),
            "\u{feff}a,\"two\nlines\"\r\nb,c\r\n"
        );
    }

    #[test]
    fn presets_and_line_ends_parse_their_names() {
        for p in Preset::ALL {
            assert_eq!(Preset::parse(p.name()), Some(p));
        }
        assert_eq!(Preset::parse("excel"), None);
        assert_eq!(LineEnd::parse("crlf"), Some(LineEnd::Crlf));
        assert_eq!(LineEnd::parse("lf"), Some(LineEnd::Lf));
        assert_eq!(LineEnd::parse("cr"), None);
    }

    #[test]
    fn fields_are_quoted_exactly_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field(" lead"), "\" lead\"");
        assert_eq!(csv_field("trail "), "\"trail \"");
        assert_eq!(csv_field("two\nlines"), "\"two\nlines\"");
        assert_eq!(csv_field(""), "");
    }
}
