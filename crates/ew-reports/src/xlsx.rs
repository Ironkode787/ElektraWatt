// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The own XLSX writer: [`xlsx()`] writes [`Table`]s as an Office
//! Open XML workbook whose bytes depend on the tables and the [`WorkbookMeta`] only.
//!
//! **The container** is the deterministic ZIP of `ew-zip` (entries in the given order, raw deflate at level 6, the DOS
//! date 1980-01-01 00:00, its own CRC-32, no ZIP64), so no ZIP or XLSX crate enters the graph. The entries, in this
//! order: `[Content_Types].xml`, `_rels/.rels`, `docProps/core.xml`, `docProps/app.xml`, `xl/workbook.xml`,
//! `xl/_rels/workbook.xml.rels`, `xl/styles.xml`, then `xl/worksheets/sheet<n>.xml` in sheet order, the hidden `_ew`
//! sheet last. No shared strings: text is written as inline strings (`t="inlineStr"`), and no `<f>` element is ever
//! written, so a text such as `=HYPERLINK(…)` stays text (NFR-SEC-17).
//!
//! **A table's sheet** (D4): column A is the `id` column (header `id`, the row's ID text, hidden, width 0); the table's
//! columns follow with their labels; row 1 is bold and frozen, with an autofilter over the header; each column is
//! `max(header, longest cell)` characters wide, clamped to [`MIN_WIDTH`]..=[`MAX_WIDTH`] (a rule, not a fit);
//! [`TableCell::Number`] cells are numeric cells, [`TableCell::Date`] cells date serials with the style `yyyy-mm-dd`
//! (a date before 1900-03-01, where Excel's serials are not the calendar's, stays text), every other cell an inline
//! string. A number is a numeric cell only when it has at most [`MAX_NUMBER_DIGITS`] significant digits, which a
//! spreadsheet's double holds exactly; a longer one (`9007199254740993`) stays text. The sheet's name is the
//! table's with `[]:*?/\`, control characters and an apostrophe at either end replaced by `_`, cut to 31 characters; a
//! name a sheet already has (ignoring case, `_ew` and `_ew_base` included) gets " (2)", " (3)", ….
//!
//! **The `_ew` sheet** (D5), hidden: key/value rows `format` `ew-table/1`, `project`, `table` (the first table's key),
//! `rev`, `generation` (empty without a store), `locale` `neutral`, `ew` (the program version), then one row per column
//! of the first table: `column`, key, label, type ([`CellType`] name), editable (`true`/`false`).
//!
//! **The `_ew_base` sheet**, hidden, after `_ew`, exactly when the first table has an editable
//! column: a header row `id` and the editable columns' keys, then each row with an ID as exported (its ID, then its
//! editable cells), then a last row `sha256` and [`WorkbookMeta::base_sha256`], the lowercase-hex SHA-256 of
//! [`base_text`]. The writer takes no hash: the caller computes the digest, and the writer refuses a workbook with an
//! editable column and no digest ([`XlsxError::NoBaseDigest`]).
//!
//! **Metadata** (doc 09 §4.3 rule 1): `docProps/core.xml` carries [`WorkbookMeta::date`] as its created and modified
//! date (the caller's `SOURCE_DATE_EPOCH`, else the model's edited date, as the PDF; never the wall clock);
//! `docProps/app.xml` names "ElektraWatt" and no version.
//!
//! **Text in XML.** `&`, `<`, `>` and `"` are escaped as entities; a character XML 1.0 cannot hold or does not keep (a
//! control character other than tab and LF, CR included, which a parser turns into LF; U+FFFE, U+FFFF) is written
//! `_xHHHH_`, and a literal `_` that would read as such an escape is written `_x005F_`, the workbook's own escape
//! (ECMA-376 Part 1, §22.9.2.19).

use std::fmt;
use std::fmt::Write as _;

use ew_zip::{Method, ZipError};

use crate::rowset::CellType;
use crate::table::{Table, TableCell};

/// The narrowest column, in characters.
pub const MIN_WIDTH: usize = 6;

/// The widest column, in characters.
pub const MAX_WIDTH: usize = 60;

/// The most rows of a sheet, the header included (Excel's limit).
pub const MAX_SHEET_ROWS: usize = 1_048_576;

/// The most columns of a sheet, the `id` column included (Excel's limit).
pub const MAX_SHEET_COLUMNS: usize = 16_384;

/// The longest sheet name (Excel's limit).
const MAX_SHEET_NAME: usize = 31;

/// The name of the hidden sheet of the round trip (D5).
pub const EW_SHEET: &str = "_ew";

/// The name of the hidden sheet of an editable table's cells as exported (D5).
pub const BASE_SHEET: &str = "_ew_base";

/// The first cell of the last row of `_ew_base`, the digest's row.
pub const BASE_DIGEST_ROW: &str = "sha256";

/// The most significant digits of a number written as a numeric cell (a double holds 15 exactly).
pub const MAX_NUMBER_DIGITS: usize = 15;

/// The `format` value of the `_ew` sheet.
pub const EW_FORMAT: &str = "ew-table/1";

/// A date and time (UTC) of the workbook's metadata.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DateTime {
    /// Year, 1 to 9999.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day, 1 to 31.
    pub day: u8,
    /// Hour, 0 to 23.
    pub hour: u8,
    /// Minute, 0 to 59.
    pub minute: u8,
    /// Second, 0 to 59.
    pub second: u8,
}

/// What a workbook says about where its tables come from (D3, D5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorkbookMeta {
    /// The export date: the created and modified date of `docProps/core.xml`.
    pub date: DateTime,
    /// The project's ID (`_ew`'s `project`).
    pub project: String,
    /// The session revision of the snapshot (`_ew`'s `rev`).
    pub rev: u64,
    /// The store generation (`_ew`'s `generation`); `None` for a model without a store.
    pub generation: Option<u64>,
    /// The program version (`_ew`'s `ew`).
    pub version: String,
    /// The lowercase-hex SHA-256 of the first table's [`base_text`] (`_ew_base`'s last row): needed exactly when the
    /// table has an editable column, and made by the caller (the writer takes no hash).
    pub base_sha256: Option<String>,
}

/// Why a workbook was not written.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum XlsxError {
    /// A table has more rows than a sheet holds (the sheet's name, its rows with the header).
    TooManyRows(String, usize),
    /// A table has more columns than a sheet holds (the sheet's name, its columns with the `id` column).
    TooManyColumns(String, usize),
    /// The container refused the entries (a part beyond the sizes of a ZIP without ZIP64).
    Zip(ZipError),
    /// The first table has an editable column and the metadata no digest of its `_ew_base` sheet (the table's name).
    NoBaseDigest(String),
}

impl fmt::Display for XlsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XlsxError::TooManyRows(sheet, n) => write!(
                f,
                "the sheet `{sheet}` would have {n} rows; a sheet holds at most {MAX_SHEET_ROWS}: export fewer rows, \
                 or as CSV"
            ),
            XlsxError::TooManyColumns(sheet, n) => write!(
                f,
                "the sheet `{sheet}` would have {n} columns; a sheet holds at most {MAX_SHEET_COLUMNS}: export fewer \
                 columns, or as CSV"
            ),
            XlsxError::Zip(e) => write!(
                f,
                "the workbook cannot be written: {e}; export fewer rows, or as CSV"
            ),
            XlsxError::NoBaseDigest(table) => write!(
                f,
                "the table `{table}` has editable columns but no digest of its exported cells; this is a bug in the \
                 program: report it, and export as CSV meanwhile"
            ),
        }
    }
}

impl std::error::Error for XlsxError {}

impl From<ZipError> for XlsxError {
    fn from(e: ZipError) -> XlsxError {
        XlsxError::Zip(e)
    }
}

/// The workbook of `tables` (one sheet each, in order, then the hidden `_ew`), as the module documentation says. An
/// empty list writes one empty sheet `Table`.
///
/// # Errors
///
/// [`XlsxError::TooManyRows`] or [`XlsxError::TooManyColumns`] for a table larger than a sheet; [`XlsxError::Zip`]
/// for a part beyond the sizes of a ZIP without ZIP64; [`XlsxError::NoBaseDigest`] for a first table with an editable
/// column and no [`WorkbookMeta::base_sha256`].
pub fn xlsx(tables: &[Table], meta: &WorkbookMeta) -> Result<Vec<u8>, XlsxError> {
    let empty;
    let tables = if tables.is_empty() {
        empty = [Table {
            name: "Table".to_owned(),
            key: String::new(),
            columns: Vec::new(),
            rows: Vec::new(),
        }];
        &empty[..]
    } else {
        tables
    };
    let names = sheet_names(tables);
    let mut sheets: Vec<String> = Vec::with_capacity(tables.len() + 1);
    for (i, (table, name)) in tables.iter().zip(&names).enumerate() {
        let rows = table.rows.len() + 1;
        if rows > MAX_SHEET_ROWS {
            return Err(XlsxError::TooManyRows(name.clone(), rows));
        }
        let columns = table.columns.len() + 1;
        if columns > MAX_SHEET_COLUMNS {
            return Err(XlsxError::TooManyColumns(name.clone(), columns));
        }
        sheets.push(table_sheet(table, i == 0));
    }
    sheets.push(ew_sheet(&tables[0], meta));
    let mut all_names: Vec<&str> = names.iter().map(String::as_str).collect();
    all_names.push(EW_SHEET);
    if tables[0].columns.iter().any(|c| c.editable) {
        let Some(digest) = meta.base_sha256.as_deref() else {
            return Err(XlsxError::NoBaseDigest(tables[0].name.clone()));
        };
        sheets.push(base_sheet(&tables[0], digest));
        all_names.push(BASE_SHEET);
    }

    let paths: Vec<String> = (1..=sheets.len())
        .map(|n| format!("xl/worksheets/sheet{n}.xml"))
        .collect();
    let content_types = content_types(sheets.len());
    let core = core_xml(meta.date);
    let workbook = workbook_xml(tables, &all_names);
    let workbook_rels = workbook_rels(sheets.len());
    let mut entries: Vec<(&str, Method, &[u8])> = vec![
        ("[Content_Types].xml", Method::Deflated, content_types.as_bytes()),
        ("_rels/.rels", Method::Deflated, ROOT_RELS.as_bytes()),
        ("docProps/core.xml", Method::Deflated, core.as_bytes()),
        ("docProps/app.xml", Method::Deflated, APP_XML.as_bytes()),
        ("xl/workbook.xml", Method::Deflated, workbook.as_bytes()),
        (
            "xl/_rels/workbook.xml.rels",
            Method::Deflated,
            workbook_rels.as_bytes(),
        ),
        ("xl/styles.xml", Method::Deflated, STYLES_XML.as_bytes()),
    ];
    for (path, sheet) in paths.iter().zip(&sheets) {
        entries.push((path, Method::Deflated, sheet.as_bytes()));
    }
    Ok(ew_zip::write(&entries)?)
}

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

const ROOT_RELS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>",
    "<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>",
    "<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>",
    "</Relationships>"
);

const APP_XML: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" ",
    "xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">",
    "<Application>ElektraWatt</Application></Properties>"
);

/// Three cell formats: 0 the default, 1 bold (the header), 2 the date `yyyy-mm-dd`.
const STYLES_XML: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">",
    "<numFmts count=\"1\"><numFmt numFmtId=\"164\" formatCode=\"yyyy\\-mm\\-dd\"/></numFmts>",
    "<fonts count=\"2\">",
    "<font><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>",
    "<font><b/><sz val=\"11\"/><name val=\"Calibri\"/><family val=\"2\"/></font>",
    "</fonts>",
    "<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills>",
    "<borders count=\"1\"><border><left/><right/><top/><bottom/><diagonal/></border></borders>",
    "<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>",
    "<cellXfs count=\"3\">",
    "<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>",
    "<xf numFmtId=\"0\" fontId=\"1\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyFont=\"1\"/>",
    "<xf numFmtId=\"164\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\" applyNumberFormat=\"1\"/>",
    "</cellXfs>",
    "<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>",
    "</styleSheet>"
);

/// The style of the header row.
const BOLD: u8 = 1;
/// The style of a date cell.
const DATE: u8 = 2;

fn content_types(sheets: usize) -> String {
    let mut s = String::from(XML_DECL);
    s.push_str("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">");
    s.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    s.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    s.push_str("<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>");
    for n in 1..=sheets {
        let _ = write!(
            s,
            "<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"
        );
    }
    s.push_str("<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>");
    s.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>");
    s.push_str("<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>");
    s.push_str("</Types>");
    s
}

fn core_xml(d: DateTime) -> String {
    let date = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        d.year, d.month, d.day, d.hour, d.minute, d.second
    );
    let mut s = String::from(XML_DECL);
    s.push_str(
        "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
         xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
    );
    let _ = write!(
        s,
        "<dcterms:created xsi:type=\"dcterms:W3CDTF\">{date}</dcterms:created>\
         <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{date}</dcterms:modified>"
    );
    s.push_str("</cp:coreProperties>");
    s
}

fn workbook_xml(tables: &[Table], names: &[&str]) -> String {
    let mut s = String::from(XML_DECL);
    let _ = write!(s, "<workbook xmlns=\"{MAIN_NS}\" xmlns:r=\"{REL_NS}\">");
    s.push_str("<bookViews><workbookView activeTab=\"0\"/></bookViews><sheets>");
    for (i, name) in names.iter().enumerate() {
        let n = i + 1;
        let hidden = if i >= tables.len() {
            " state=\"hidden\""
        } else {
            ""
        };
        let _ = write!(
            s,
            "<sheet name=\"{}\" sheetId=\"{n}\"{hidden} r:id=\"rId{n}\"/>",
            escape(name)
        );
    }
    s.push_str("</sheets><definedNames>");
    for (i, table) in tables.iter().enumerate() {
        let _ = write!(
            s,
            "<definedName name=\"_xlnm._FilterDatabase\" localSheetId=\"{i}\" hidden=\"1\">{}</definedName>",
            escape(&format!(
                "'{}'!$A$1:${}${}",
                names[i].replace('\'', "''"),
                column_name(table.columns.len()),
                table.rows.len() + 1
            ))
        );
    }
    s.push_str("</definedNames></workbook>");
    s
}

fn workbook_rels(sheets: usize) -> String {
    let mut s = String::from(XML_DECL);
    s.push_str("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    for n in 1..=sheets {
        let _ = write!(
            s,
            "<Relationship Id=\"rId{n}\" Type=\"{REL_NS}/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"
        );
    }
    let _ = write!(
        s,
        "<Relationship Id=\"rId{}\" Type=\"{REL_NS}/styles\" Target=\"styles.xml\"/>",
        sheets + 1
    );
    s.push_str("</Relationships>");
    s
}

/// The sheet of a table (D4); `first`: the selected sheet.
fn table_sheet(table: &Table, first: bool) -> String {
    let last = format!("{}{}", column_name(table.columns.len()), table.rows.len() + 1);
    let mut s = String::from(XML_DECL);
    let _ = write!(s, "<worksheet xmlns=\"{MAIN_NS}\" xmlns:r=\"{REL_NS}\">");
    let _ = write!(s, "<dimension ref=\"A1:{last}\"/>");
    let selected = if first { " tabSelected=\"1\"" } else { "" };
    let _ = write!(
        s,
        "<sheetViews><sheetView{selected} workbookViewId=\"0\">\
         <pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/>\
         <selection pane=\"bottomLeft\" activeCell=\"A2\" sqref=\"A2\"/></sheetView></sheetViews>"
    );
    s.push_str("<sheetFormatPr defaultRowHeight=\"15\"/>");
    s.push_str("<cols><col min=\"1\" max=\"1\" width=\"0\" hidden=\"1\" customWidth=\"1\"/>");
    for (i, column) in table.columns.iter().enumerate() {
        let longest = table
            .rows
            .iter()
            .filter_map(|r| r.cells.get(i))
            .map(|c| c.text().chars().count())
            .fold(column.label.chars().count(), usize::max);
        let width = longest.clamp(MIN_WIDTH, MAX_WIDTH);
        let n = i + 2;
        let _ = write!(
            s,
            "<col min=\"{n}\" max=\"{n}\" width=\"{width}\" customWidth=\"1\"/>"
        );
    }
    s.push_str("</cols><sheetData>");
    s.push_str("<row r=\"1\">");
    inline(&mut s, "A1", "id", Some(BOLD));
    for (i, column) in table.columns.iter().enumerate() {
        inline(
            &mut s,
            &format!("{}1", column_name(i + 1)),
            &column.label,
            Some(BOLD),
        );
    }
    s.push_str("</row>");
    for (r, row) in table.rows.iter().enumerate() {
        let n = r + 2;
        let _ = write!(s, "<row r=\"{n}\">");
        if let Some(id) = row.id.as_deref().filter(|id| !id.is_empty()) {
            inline(&mut s, &format!("A{n}"), id, None);
        }
        for (i, cell) in row.cells.iter().enumerate() {
            let at = format!("{}{n}", column_name(i + 1));
            match cell {
                TableCell::Text(t) if t.is_empty() => {}
                TableCell::Number(t) if is_number(t) && significant_digits(t) <= MAX_NUMBER_DIGITS => {
                    let _ = write!(s, "<c r=\"{at}\"><v>{t}</v></c>");
                }
                TableCell::Date(t) => match date_serial(t) {
                    Some(serial) => {
                        let _ = write!(s, "<c r=\"{at}\" s=\"{DATE}\"><v>{serial}</v></c>");
                    }
                    None => inline(&mut s, &at, t, None),
                },
                other => inline(&mut s, &at, other.text(), None),
            }
        }
        s.push_str("</row>");
    }
    s.push_str("</sheetData>");
    let _ = write!(s, "<autoFilter ref=\"A1:{last}\"/>");
    s.push_str("</worksheet>");
    s
}

/// The hidden `_ew` sheet (D5) of a workbook whose first table is `first`.
fn ew_sheet(first: &Table, meta: &WorkbookMeta) -> String {
    let mut rows: Vec<Vec<String>> = vec![
        vec!["format".into(), EW_FORMAT.into()],
        vec!["project".into(), meta.project.clone()],
        vec!["table".into(), first.key.clone()],
        vec!["rev".into(), meta.rev.to_string()],
        vec![
            "generation".into(),
            meta.generation.map(|g| g.to_string()).unwrap_or_default(),
        ],
        vec!["locale".into(), "neutral".into()],
        vec!["ew".into(), meta.version.clone()],
    ];
    for c in &first.columns {
        rows.push(vec![
            "column".into(),
            c.key.clone(),
            c.label.clone(),
            cell_type_name(c.cell).into(),
            if c.editable { "true" } else { "false" }.into(),
        ]);
    }
    text_sheet(&rows)
}

/// The rows of the `_ew_base` sheet of `table` without its digest row: the header `id` and the editable columns' keys,
/// then each row with an ID, its ID and its editable cells.
fn base_rows(table: &Table) -> Vec<Vec<String>> {
    let editable: Vec<usize> = (0..table.columns.len())
        .filter(|&i| table.columns[i].editable)
        .collect();
    let header: Vec<String> = std::iter::once("id".to_owned())
        .chain(editable.iter().map(|&i| table.columns[i].key.clone()))
        .collect();
    let mut rows = vec![header];
    for row in &table.rows {
        let Some(id) = row.id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        rows.push(
            std::iter::once(id.to_owned())
                .chain(
                    editable
                        .iter()
                        .map(|&i| row.cells.get(i).map_or("", TableCell::text).to_owned()),
                )
                .collect(),
        );
    }
    rows
}

/// The text the digest of the `_ew_base` sheet is made over: its rows without the digest row,
/// each row's cell texts joined by tabs and ended by LF (the header first, then each row with an ID: its ID and its
/// editable cells). The round trip reads the sheet back, makes the same text ([`rows_text`]) and compares the digests,
/// so an edited base is detected.
pub fn base_text(table: &Table) -> String {
    rows_text(&base_rows(table))
}

/// `rows` as tab-separated cell texts, each row ended by LF.
pub fn rows_text(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&row.join("\t"));
        out.push('\n');
    }
    out
}

/// The hidden `_ew_base` sheet of `table` with its digest row.
fn base_sheet(table: &Table, digest: &str) -> String {
    let mut rows = base_rows(table);
    rows.push(vec![BASE_DIGEST_ROW.to_owned(), digest.to_owned()]);
    text_sheet(&rows)
}

/// A sheet of inline-string cells (an empty text is no cell).
fn text_sheet(rows: &[Vec<String>]) -> String {
    let mut s = String::from(XML_DECL);
    let _ = write!(
        s,
        "<worksheet xmlns=\"{MAIN_NS}\" xmlns:r=\"{REL_NS}\"><sheetData>"
    );
    for (r, row) in rows.iter().enumerate() {
        let n = r + 1;
        let _ = write!(s, "<row r=\"{n}\">");
        for (i, text) in row.iter().enumerate() {
            if !text.is_empty() {
                inline(&mut s, &format!("{}{n}", column_name(i)), text, None);
            }
        }
        s.push_str("</row>");
    }
    s.push_str("</sheetData></worksheet>");
    s
}

/// The name of a [`CellType`] in the `_ew` sheet.
pub fn cell_type_name(t: CellType) -> &'static str {
    match t {
        CellType::Text => "Text",
        CellType::Ident => "Ident",
        CellType::Number => "Number",
        CellType::Tag => "Tag",
        CellType::Folio => "Folio",
        CellType::Folios => "Folios",
        CellType::CellRef => "CellRef",
        CellType::Qty => "Qty",
        CellType::Colour => "Colour",
        CellType::Address => "Address",
        CellType::Date => "Date",
        CellType::Bool => "Bool",
        CellType::Range => "Range",
    }
}

/// Appends an inline-string cell.
fn inline(s: &mut String, at: &str, text: &str, style: Option<u8>) {
    let style = style.map(|n| format!(" s=\"{n}\"")).unwrap_or_default();
    let space = if text.starts_with(char::is_whitespace) || text.ends_with(char::is_whitespace) {
        " xml:space=\"preserve\""
    } else {
        ""
    };
    let _ = write!(
        s,
        "<c r=\"{at}\"{style} t=\"inlineStr\"><is><t{space}>{}</t></is></c>",
        escape(text)
    );
}

/// The letters of the 0-based column `i` (`0` → `A`, `26` → `AA`).
pub fn column_name(i: usize) -> String {
    let mut n = i + 1;
    let mut out = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        out.push(b'A' + u8::try_from(rem).unwrap_or(0));
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// The significant digits of a number text as [`is_number`] takes it: its digits without the leading and trailing
/// zeros (`0.0015` has 2, `1500` 2, `9007199254740993` 16; zero has 1).
fn significant_digits(t: &str) -> usize {
    let digits: String = t.chars().filter(char::is_ascii_digit).collect();
    digits.trim_start_matches('0').trim_end_matches('0').len().max(1)
}

/// Whether `t` is a number as [`TableCell::Number`] holds it: an optional `-`, digits, and an optional `.` with digits.
fn is_number(t: &str) -> bool {
    let body = t.strip_prefix('-').unwrap_or(t);
    let (int, frac) = body.split_once('.').map_or((body, None), |(a, b)| (a, Some(b)));
    !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
}

/// The Excel serial of a `yyyy-mm-dd` date (days since 1899-12-30); `None` for a text that is not such a date or a
/// date before 1900-03-01 (Excel counts a 1900-02-29 that the calendar has not).
fn date_serial(t: &str) -> Option<i64> {
    let b = t.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let part = t.get(r)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let serial = days_from_civil(y, m, d) - days_from_civil(1899, 12, 30);
    (serial >= 61).then_some(serial)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The valid sheet names of `tables`, in order: forbidden characters and an apostrophe at either end replaced, cut to
/// 31 characters, and made unique (ignoring case by `str::to_lowercase`, with `_ew` and `_ew_base` taken) by " (2)",
/// " (3)", ….
fn sheet_names(tables: &[Table]) -> Vec<String> {
    let mut taken: Vec<String> = vec![EW_SHEET.to_lowercase(), BASE_SHEET.to_lowercase()];
    let mut out = Vec::with_capacity(tables.len());
    for t in tables {
        let count = t.name.chars().count();
        let clean: String = t
            .name
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let end = i == 0 || i + 1 == count;
                if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\')
                    || c.is_control()
                    || (end && c == '\'')
                {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        let clean = if clean.trim().is_empty() {
            "Sheet".to_owned()
        } else {
            clean
        };
        // A cut may leave an apostrophe at the end, which a sheet name cannot have.
        let cut = |keep: usize| -> String {
            let mut name: String = clean.chars().take(keep).collect();
            if name.ends_with('\'') {
                name.pop();
                name.push('_');
            }
            name
        };
        let mut name = cut(MAX_SHEET_NAME);
        let mut n = 2;
        while taken.contains(&name.to_lowercase()) {
            let suffix = format!(" ({n})");
            let keep = MAX_SHEET_NAME - suffix.chars().count();
            name = cut(keep) + &suffix;
            n += 1;
        }
        taken.push(name.to_lowercase());
        out.push(name);
    }
    out
}

/// `text` as XML character data or an attribute value: entities for `&<>"`, `_xHHHH_` for the characters XML cannot
/// hold, `_x005F_` for a `_` that would read as such an escape.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (i, c) in text.char_indices() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' => out.push(c),
            c if c.is_control() || c == '\u{FFFE}' || c == '\u{FFFF}' => {
                let _ = write!(out, "_x{:04X}_", u32::from(c));
            }
            '_' if looks_escaped(&text[i..]) => out.push_str("_x005F_"),
            c => out.push(c),
        }
    }
    out
}

/// Whether `s` starts with `_xHHHH_`.
fn looks_escaped(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 7 && b[1] == b'x' && b[2..6].iter().all(u8::is_ascii_hexdigit) && b[6] == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::{TableColumn, TableRow};

    fn table(name: &str) -> Table {
        Table {
            name: name.to_owned(),
            key: "sql".to_owned(),
            columns: vec![TableColumn {
                key: "a".into(),
                label: "A".into(),
                cell: CellType::Text,
                editable: false,
            }],
            rows: vec![TableRow {
                id: None,
                heading: false,
                cells: vec![TableCell::Text("x".into())],
            }],
        }
    }

    #[test]
    fn column_names_count_like_a_spreadsheet() {
        assert_eq!(column_name(0), "A");
        assert_eq!(column_name(25), "Z");
        assert_eq!(column_name(26), "AA");
        assert_eq!(column_name(701), "ZZ");
        assert_eq!(column_name(702), "AAA");
    }

    #[test]
    fn sheet_names_are_cleaned_cut_and_unique() {
        let long = "abcdefghijklmnopqrstuvwxyz0123456789";
        let names = sheet_names(&[
            table("a/b:c"),
            table("A_B_C"),
            table(long),
            table(long),
            table("_EW"),
            table(""),
        ]);
        assert_eq!(
            names,
            vec![
                "a_b_c".to_owned(),
                "A_B_C (2)".to_owned(),
                "abcdefghijklmnopqrstuvwxyz01234".to_owned(),
                "abcdefghijklmnopqrstuvwxyz0 (2)".to_owned(),
                "_EW (2)".to_owned(),
                "Sheet".to_owned(),
            ]
        );
    }

    #[test]
    fn text_escapes_entities_controls_and_lookalike_escapes() {
        assert_eq!(escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
        assert_eq!(escape("bell\u{7}"), "bell_x0007_");
        assert_eq!(escape("_x0041_ and _x"), "_x005F_x0041_ and _x");
        assert_eq!(escape("tab\tline\n"), "tab\tline\n");
        // A carriage return is the workbook's escape, which a parser keeps.
        assert_eq!(escape("a\r\nb"), "a_x000D_\nb");
    }

    /// Names are unique ignoring case as `str::to_lowercase` folds it, and an apostrophe
    /// at either end is replaced (also one a cut leaves at the end).
    #[test]
    fn sheet_names_fold_case_fully_and_drop_end_apostrophes() {
        let names = sheet_names(&[
            table("ÄRGER"),
            table("ärger"),
            table("'quoted'"),
            table("it's"),
            table("_EW_BASE"),
            table("abcdefghijklmnopqrstuvwxyz0123'xyz"),
        ]);
        assert_eq!(
            names,
            vec![
                "ÄRGER".to_owned(),
                "ärger (2)".to_owned(),
                "_quoted_".to_owned(),
                "it's".to_owned(),
                "_EW_BASE (2)".to_owned(),
                "abcdefghijklmnopqrstuvwxyz0123_".to_owned(),
            ]
        );
    }

    /// A number of more than 15 significant digits stays text, a shorter one is numeric.
    #[test]
    fn a_long_number_stays_text() {
        let mut t = table("n");
        t.columns[0].cell = CellType::Number;
        t.rows[0].cells[0] = TableCell::Number("9007199254740993".into());
        t.rows.push(TableRow {
            id: None,
            heading: false,
            cells: vec![TableCell::Number("123456789012345".into())],
        });
        t.rows.push(TableRow {
            id: None,
            heading: false,
            cells: vec![TableCell::Number("-0.001500".into())],
        });
        let sheet = table_sheet(&t, true);
        assert!(
            sheet.contains("<c r=\"B2\" t=\"inlineStr\"><is><t>9007199254740993</t></is></c>"),
            "{sheet}"
        );
        assert!(
            sheet.contains("<c r=\"B3\"><v>123456789012345</v></c>"),
            "{sheet}"
        );
        assert!(sheet.contains("<c r=\"B4\"><v>-0.001500</v></c>"), "{sheet}");
        assert_eq!(significant_digits("0"), 1);
        assert_eq!(significant_digits("1500"), 2);
    }

    /// Every refusal is whole and ends with what to do.
    #[test]
    fn every_refusal_says_what_to_do() {
        assert_eq!(
            XlsxError::TooManyRows("wire_list".into(), 1_048_577).to_string(),
            "the sheet `wire_list` would have 1048577 rows; a sheet holds at most 1048576: export fewer rows, or as CSV"
        );
        assert_eq!(
            XlsxError::TooManyColumns("sql".into(), 16_385).to_string(),
            "the sheet `sql` would have 16385 columns; a sheet holds at most 16384: export fewer columns, or as CSV"
        );
        assert_eq!(
            XlsxError::Zip(ZipError::TooLarge).to_string(),
            format!(
                "the workbook cannot be written: {}; export fewer rows, or as CSV",
                ZipError::TooLarge
            )
        );
        assert_eq!(
            XlsxError::NoBaseDigest("-X1031".into()).to_string(),
            "the table `-X1031` has editable columns but no digest of its exported cells; this is a bug in the \
             program: report it, and export as CSV meanwhile"
        );
    }

    fn meta(base: Option<&str>) -> WorkbookMeta {
        WorkbookMeta {
            date: DateTime {
                year: 2026,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
            },
            project: "p".into(),
            rev: 1,
            generation: None,
            version: "0".into(),
            base_sha256: base.map(str::to_owned),
        }
    }

    /// D5: an editable table carries `_ew_base` after `_ew`, hidden, with the rows of
    /// [`base_text`] and the digest row; without a digest the writer refuses; a table without an editable column has
    /// no `_ew_base`.
    #[test]
    fn an_editable_table_carries_its_base() {
        let mut t = table("T");
        t.columns.push(TableColumn {
            key: "b".into(),
            label: "B".into(),
            cell: CellType::Text,
            editable: true,
        });
        t.rows[0].id = Some("e:1".into());
        t.rows[0].cells.push(TableCell::Text("old".into()));
        t.rows.push(TableRow {
            id: None,
            heading: true,
            cells: vec![TableCell::Text("head".into()), TableCell::Text(String::new())],
        });
        assert_eq!(base_text(&t), "id\tb\ne:1\told\n");
        assert_eq!(
            xlsx(std::slice::from_ref(&t), &meta(None)),
            Err(XlsxError::NoBaseDigest("T".into()))
        );
        let bytes = xlsx(std::slice::from_ref(&t), &meta(Some("abc"))).unwrap();
        let entries = ew_zip::read(&bytes, &ew_zip::ZipLimits::default()).unwrap();
        let text = |name: &str| {
            String::from_utf8(entries.iter().find(|e| e.name == name).unwrap().data.clone()).unwrap()
        };
        let book = text("xl/workbook.xml");
        assert!(book.contains("<sheet name=\"_ew\" sheetId=\"2\" state=\"hidden\" r:id=\"rId2\"/>"));
        assert!(book.contains("<sheet name=\"_ew_base\" sheetId=\"3\" state=\"hidden\" r:id=\"rId3\"/>"));
        let base = text("xl/worksheets/sheet3.xml");
        assert!(
            base.contains("<c r=\"A3\" t=\"inlineStr\"><is><t>sha256</t></is></c>"),
            "{base}"
        );
        assert!(
            base.contains("<c r=\"B3\" t=\"inlineStr\"><is><t>abc</t></is></c>"),
            "{base}"
        );
        t.columns[1].editable = false;
        let plain = xlsx(&[t], &meta(None)).unwrap();
        let entries = ew_zip::read(&plain, &ew_zip::ZipLimits::default()).unwrap();
        assert!(!entries.iter().any(|e| e.name == "xl/worksheets/sheet3.xml"));
    }

    #[test]
    fn numbers_and_dates_are_recognised() {
        for good in ["0", "1", "-3", "1.71", "28"] {
            assert!(is_number(good), "{good}");
        }
        for bad in ["", "-", "1.", ".5", "1,5", "1e3", "1.5 mm²"] {
            assert!(!is_number(bad), "{bad}");
        }
        assert_eq!(date_serial("1900-03-01"), Some(61));
        assert_eq!(date_serial("2026-01-01"), Some(46_023));
        assert_eq!(date_serial("1900-02-28"), None);
        assert_eq!(date_serial("01-01-2026"), None);
    }

    /// TX-4 (NFR-SEC-17): a text that looks like a formula is an inline string, and no part has an `<f>` element.
    #[test]
    fn a_formula_text_stays_an_inline_string() {
        let mut t = table("Formula");
        t.rows[0].cells[0] = TableCell::Text("=HYPERLINK(\"x\")".into());
        let sheet = table_sheet(&t, true);
        assert!(
            sheet.contains("<c r=\"B2\" t=\"inlineStr\"><is><t>=HYPERLINK(&quot;x&quot;)</t></is></c>"),
            "{sheet}"
        );
        assert!(!sheet.contains("<f>") && !sheet.contains("<f "));
    }
}
