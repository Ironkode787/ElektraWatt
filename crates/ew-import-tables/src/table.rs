// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tables by name, typed cell readers and the capped finding list.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use ew_interop_api::{Code, ImportMode, QuickFixProposal, ReportEntry, Severity, SourceLocation};
use ew_mltext::{Ident, MlText, is_nfc, to_nfc};
use ew_schema::{
    ColourCode, ColourSpec, Csa, Date, IecColour, KeyedList, Quantity, TechVal, TechValue, Unit,
};
use ew_units::{Dec3, Nm, Pt2};

use crate::MAX_REPORT_ENTRIES;
use crate::csv::{Cell, CsvTable, cell_text};

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

/// One loaded table: header map and rows of cell spans.
///
/// The table holds its input (shared, never copied) and the spans of [`CsvTable`]; a cell is normalised to NFC
/// only when it is not NFC already, and then its normalised text goes to a side buffer. Cells that are NFC are
/// never copied.
pub(crate) struct Table {
    /// The file name (`devices.csv`).
    pub(crate) file: String,
    columns: BTreeMap<String, usize>,
    input: Arc<[u8]>,
    ncols: usize,
    /// The 1-based line of each row.
    lines: Vec<u32>,
    /// The cells of all rows, row-major, `ncols` per row.
    cells: Vec<Cell>,
    /// Unescaped and NFC-normalised text.
    owned: String,
    /// Strict or lenient: what the cell readers do with a value that a strict import refuses.
    mode: ImportMode,
}

impl fmt::Debug for Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Table")
            .field("file", &self.file)
            .field("columns", &self.columns)
            .field("rows", &self.lines.len())
            .finish_non_exhaustive()
    }
}

/// The finding of a row dropped because its key is not NFC.
const KEY_NOT_NFC: &str = "row dropped: the key is not in Unicode NFC (keys are hashed byte for byte)";

impl Table {
    /// The table `csv`, parsed from `input`, of `file` under `spec` and import `mode`, or `None` when a required
    /// column is missing (reported as EW-IMP-002). Unknown columns are reported (EW-IMP-004) and ignored.
    ///
    /// Keys are hashed byte for byte (README of the golden tables, doc 14 §9 strict mode): a row whose `key`
    /// cell is not in NFC is dropped with EW-IMP-002. Every other cell of a known column is NFC-normalised,
    /// only when it is not NFC already; the table keeps the input and the spans, so nothing else is copied.
    pub(crate) fn new(
        spec: TableSpec,
        file: &str,
        input: Arc<[u8]>,
        csv: CsvTable,
        mode: ImportMode,
        findings: &mut Findings,
    ) -> Option<Table> {
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
        let CsvTable {
            ncols,
            mut lines,
            mut cells,
            mut owned,
            ..
        } = csv;
        let key_col = columns.get("key").copied();
        let used: Vec<usize> = columns
            .values()
            .copied()
            .filter(|&i| Some(i) != key_col)
            .collect();
        let mut kept = 0usize;
        for row in 0..lines.len() {
            let base = row * ncols;
            if let Some(k) = key_col {
                let key = cell_text(&input, &owned, cells[base + k]);
                if !is_nfc(key) {
                    findings.at(
                        Code::MALFORMED_ITEM,
                        Severity::Error,
                        file,
                        Some(key),
                        Some(lines[row]),
                        KEY_NOT_NFC,
                    );
                    continue;
                }
            }
            for &c in &used {
                let nfc = match to_nfc(cell_text(&input, &owned, cells[base + c])) {
                    Cow::Borrowed(_) => continue,
                    Cow::Owned(s) => s,
                };
                cells[base + c] = Cell::owned(owned.len(), nfc.len());
                owned.push_str(&nfc);
            }
            if kept != row {
                cells.copy_within(base..base + ncols, kept * ncols);
                lines[kept] = lines[row];
            }
            kept += 1;
        }
        lines.truncate(kept);
        cells.truncate(kept * ncols);
        Some(Table {
            file: file.to_owned(),
            columns,
            input,
            ncols,
            lines,
            cells,
            owned,
            mode,
        })
    }

    /// The text of `cell`.
    fn text(&self, cell: Cell) -> &str {
        cell_text(&self.input, &self.owned, cell)
    }

    /// The rows in file order.
    pub(crate) fn rows(&self) -> impl Iterator<Item = Row<'_>> {
        (0..self.lines.len()).map(move |index| Row { table: self, index })
    }

    /// The number of rows.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }
}

/// One row of a [`Table`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Row<'t> {
    table: &'t Table,
    index: usize,
}

impl<'t> Row<'t> {
    /// The cell of `column`, `None` when the column is absent or the cell empty.
    pub(crate) fn get(&self, column: &str) -> Option<&'t str> {
        let i = *self.table.columns.get(column)?;
        let cell = *self.table.cells.get(self.index * self.table.ncols + i)?;
        Some(self.table.text(cell)).filter(|s| !s.is_empty())
    }

    /// The `key` cell (empty when absent).
    pub(crate) fn key(&self) -> &'t str {
        self.get("key").unwrap_or("")
    }

    /// The 1-based line of the row.
    pub(crate) fn line(&self) -> u32 {
        self.table.lines[self.index]
    }

    /// The file name.
    pub(crate) fn file(&self) -> &'t str {
        &self.table.file
    }

    /// The import mode of the table.
    pub(crate) fn mode(&self) -> ImportMode {
        self.table.mode
    }

    /// A reader of this row's cells that remembers the first problem.
    pub(crate) fn reader(self) -> Rd<'t> {
        Rd {
            row: self,
            err: None,
            foreign: Vec::new(),
        }
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
    /// The foreign colour cells a lenient import kept, as `(column, colour)`.
    foreign: Vec<(String, ForeignColour)>,
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

    /// A colour cell (`BK`, `GNYE`, `DBU`). A foreign colour code (`BR`, [`ForeignColour`]) fails the row in a
    /// strict import; a lenient import keeps it and notes it for [`Rd::take_foreign`].
    pub(crate) fn colour(&mut self, col: &str) -> Option<ColourSpec> {
        let v = self.text(col);
        let mode = self.row.mode();
        let (spec, foreign) = self.parse(col, v, |s| parse_colour(s, mode))?;
        if let Some(f) = foreign {
            self.foreign.push((col.to_owned(), f));
        }
        Some(spec)
    }

    /// The foreign colour cells kept so far. The caller reports them with [`Findings::foreign_colours`] once
    /// the row is certain to be kept.
    pub(crate) fn take_foreign(&mut self) -> Vec<(String, ForeignColour)> {
        std::mem::take(&mut self.foreign)
    }

    /// A length in millimetres (a `Dec3` cell), in nanometres (WP-S4.01 part 2).
    pub(crate) fn mm(&mut self, col: &str) -> Option<Nm> {
        let v = self.text(col);
        self.parse(col, v, parse_mm)
    }

    /// A `;`-separated list of exactly `n` lengths in millimetres (`750;1000;420`), in nanometres; `None` when
    /// absent.
    pub(crate) fn mm_list(&mut self, col: &str, n: usize) -> Option<Vec<Nm>> {
        let items = self.list(col);
        if items.is_empty() {
            return None;
        }
        if items.len() != n {
            self.fail(format!("column `{col}`: {n} lengths in mm, not {}", items.len()));
            return None;
        }
        let v: Result<Vec<Nm>, String> = items.into_iter().map(parse_mm).collect();
        match v {
            Ok(v) => Some(v),
            Err(e) => {
                self.fail(format!("column `{col}`: {e}"));
                None
            }
        }
    }

    /// A `;`-separated list of points `u v` in millimetres (`40 740;610 740`), in nanometres (empty when absent).
    pub(crate) fn points(&mut self, col: &str) -> Vec<Pt2> {
        let mut out = Vec::new();
        for item in self.list(col) {
            let p = item
                .split_once(' ')
                .ok_or_else(|| format!("`{item}` is not a point `u v`"))
                .and_then(|(u, v)| Ok(Pt2::new(parse_mm(u)?, parse_mm(v)?)));
            match p {
                Ok(p) => out.push(p),
                Err(e) => self.fail(format!("column `{col}`: {e}")),
            }
        }
        out
    }

    /// A cross-section in mm².
    pub(crate) fn csa(&mut self, col: &str) -> Option<Csa> {
        self.dec3(col).map(Csa::Mm2)
    }

    /// Technical data (doc 03 §7, U-32): `;`-separated `<key>=<value>` items whose value is a quantity
    /// `<decimal> <unit>` (`10 A`, `15 degC`, the unit in its ASCII spelling of the format) or an identifier
    /// (`C`). A malformed item fails the row; a repeated key too.
    pub(crate) fn tech_data(&mut self, col: &str) -> KeyedList<TechValue> {
        let mut items = Vec::new();
        for item in self.list(col) {
            match parse_tech_value(item) {
                Ok(v) => items.push(v),
                Err(e) => self.fail(format!("column `{col}`: {e}")),
            }
        }
        KeyedList::from_vec(items).unwrap_or_else(|_| {
            self.fail(format!("column `{col}`: a key is repeated"));
            KeyedList::default()
        })
    }
}

/// An identifier from a cell (already NFC and trimmed by the dialect).
pub(crate) fn parse_ident(s: &str) -> Result<Ident, String> {
    s.parse::<Ident>()
        .map_err(|e| format!("`{s}` is not an identifier: {e}"))
}

pub(crate) fn parse_uint<T: TryFrom<u64>>(s: &str) -> Result<T, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("`{s}` is not an unsigned integer"));
    }
    let n: u64 = s.parse().map_err(|_| format!("`{s}` is out of range"))?;
    T::try_from(n).map_err(|_| format!("`{s}` is out of range"))
}

/// One technical value `<key>=<decimal> <unit>` or `<key>=<identifier>`.
fn parse_tech_value(item: &str) -> Result<TechValue, String> {
    let (key, value) = item
        .split_once('=')
        .ok_or_else(|| format!("`{item}` is not <key>=<value>"))?;
    let key = parse_ident(key)?;
    let value = match value.split_once(' ') {
        Some((number, unit)) => {
            let unit: Unit = serde_json::from_value(serde_json::Value::String(unit.to_owned()))
                .map_err(|e| format!("`{unit}` is not a unit: {e}"))?;
            if matches!(unit, Unit::Unknown(_)) {
                return Err(format!("`{unit:?}` is not a unit of the format"));
            }
            TechVal::Quantity(Quantity::new(parse_dec3(number)?, unit))
        }
        None => TechVal::Ident(parse_ident(value)?),
    };
    Ok(TechValue {
        key,
        value,
        extra: Default::default(),
    })
}

/// A canonical `Dec3` cell.
pub(crate) fn parse_dec3(s: &str) -> Result<Dec3, String> {
    s.parse::<Dec3>()
        .map_err(|e| format!("`{s}` is not a decimal: {e}"))
}

/// A length in millimetres given as a canonical `Dec3` (`58.6`), in nanometres (exact: a thousandth of a millimetre
/// is 1000 nm).
pub(crate) fn parse_mm(s: &str) -> Result<Nm, String> {
    let d = parse_dec3(s)?;
    d.0.checked_mul(1_000)
        .map(Nm)
        .ok_or_else(|| format!("`{s}` mm is out of range"))
}

/// The foreign colour notations of doc 14 §4.12 and the IEC 60757 codes they stand for (`BR` for `BN`, D-17).
pub const COLOUR_ALIASES: [(&str, IecColour); 7] = [
    ("BR", IecColour::Bn),
    ("SW", IecColour::Bk),
    ("BL", IecColour::Bu),
    ("RT", IecColour::Rd),
    ("GE", IecColour::Ye),
    ("WS", IecColour::Wh),
    ("GR", IecColour::Gy),
];

/// A colour cell that holds foreign codes (doc 14 §4.12): every two-letter chunk is an IEC 60757 code or an
/// alias, and at least one is an alias.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct ForeignColour {
    /// The alias chunks of the cell with their IEC code, in cell order.
    pub(crate) aliases: Vec<(&'static str, IecColour)>,
    /// The cell with every alias mapped to its IEC code.
    pub(crate) mapped: ColourSpec,
}

impl ForeignColour {
    /// The finding text after "column `x`: ": one alias names its IEC code, several list each pair.
    fn describe(&self, strict: bool) -> String {
        let (one, many) = if strict {
            ("is", "are")
        } else {
            ("kept as", "kept as")
        };
        match self.aliases.as_slice() {
            [(alias, code)] => format!(
                "`{alias}` {one} a foreign colour code (IEC 60757: {})",
                code.as_str()
            ),
            _ => {
                let pairs: Vec<String> = self
                    .aliases
                    .iter()
                    .map(|(a, c)| format!("`{a}` → `{}`", c.as_str()))
                    .collect();
                let list = match pairs.split_last() {
                    Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
                    _ => pairs.join(""),
                };
                format!("{list} {many} foreign colour codes (IEC 60757)")
            }
        }
    }
}

/// A colour cell: a run of IEC 60757 two-letter codes (`BK`, `GNYE` = GN + YE), else one company code (`DBU`).
///
/// A run whose chunks are IEC codes or foreign aliases ([`COLOUR_ALIASES`]), with at least one alias, holds a
/// foreign code: a strict import refuses the cell (EW-CORE-031), a lenient one keeps the alias chunks as
/// [`ColourCode::Foreign`] and returns the [`ForeignColour`] with the IEC mapping.
pub(crate) fn parse_colour(s: &str, mode: ImportMode) -> Result<(ColourSpec, Option<ForeignColour>), String> {
    let bytes = s.as_bytes();
    if s.is_ascii() && bytes.len() % 2 == 0 && !bytes.is_empty() {
        // Per chunk: the code as stored, the IEC code, and the alias when the chunk is one.
        type Chunk = (ColourCode, IecColour, Option<&'static str>);
        let chunks: Option<Vec<Chunk>> = bytes
            .chunks(2)
            .map(|c| {
                let c = std::str::from_utf8(c).ok()?;
                if let Ok(iec) = c.parse::<IecColour>() {
                    return Some((ColourCode::Iec(iec), iec, None));
                }
                let (alias, iec) = COLOUR_ALIASES.iter().find(|(a, _)| *a == c)?;
                let foreign = parse_ident(alias).ok()?;
                Some((ColourCode::Foreign(foreign), *iec, Some(*alias)))
            })
            .collect();
        if let Some(chunks) = chunks {
            let aliases: Vec<(&'static str, IecColour)> = chunks
                .iter()
                .filter_map(|(_, iec, alias)| alias.map(|a| (a, *iec)))
                .collect();
            let spec = |codes: Vec<ColourCode>| ColourSpec::new(codes).map_err(|e| format!("`{s}`: {e}"));
            if aliases.is_empty() {
                return Ok((spec(chunks.into_iter().map(|c| c.0).collect())?, None));
            }
            let found = ForeignColour {
                aliases,
                mapped: spec(chunks.iter().map(|c| ColourCode::Iec(c.1)).collect())?,
            };
            return match mode {
                ImportMode::Strict => Err(format!(
                    "{}; a strict import stores no foreign code (EW-CORE-031)",
                    found.describe(true)
                )),
                ImportMode::Lenient => Ok((spec(chunks.into_iter().map(|c| c.0).collect())?, Some(found))),
            };
        }
    }
    let code = parse_ident(s)?;
    let code = ColourCode::company(code).map_err(|e| format!("`{s}`: {e}"))?;
    Ok((ColourSpec::one(code), None))
}

/// The quick fix target of a foreign colour: the `fields.set` kind and path, and the record's UUID.
pub(crate) struct ColourFix {
    /// The record kind of the command (`conductor`, `connection`).
    pub(crate) kind: &'static str,
    /// The field path (`colour`, `overrides.colour`).
    pub(crate) path: &'static str,
    /// The record's UUID.
    pub(crate) target: String,
}

impl Findings {
    /// EW-IMP-008 (warning) for each foreign colour cell of the kept row `row`: the alias is kept as a foreign
    /// code. `fix` adds the quick fix "use <IEC codes>" (a declarative `fields.set` proposal), for the tables
    /// whose records a command can edit.
    pub(crate) fn foreign_colours(
        &mut self,
        row: Row<'_>,
        found: Vec<(String, ForeignColour)>,
        fix: Option<ColourFix>,
    ) {
        for (column, colour) in found {
            let message = format!("column `{column}`: {}", colour.describe(false));
            let location = SourceLocation {
                file: row.file().to_owned(),
                item: Some(row.key().to_owned()),
                line: Some(row.line()),
                ..SourceLocation::default()
            };
            let mut entry = ReportEntry::new(Code::FOREIGN_VALUE, Severity::Warning, message).at(location);
            if let (Some(fix), Ok(value)) = (&fix, serde_json::to_value(&colour.mapped)) {
                let codes: Vec<&str> = colour.aliases.iter().map(|(_, c)| c.as_str()).collect();
                entry = entry.with_fix(QuickFixProposal {
                    label: format!("use {}", codes.join(", ")),
                    command: "fields.set".to_owned(),
                    params: serde_json::json!({
                        "kind": fix.kind,
                        "targets": [fix.target],
                        "path": fix.path,
                        "value": value,
                    }),
                });
            }
            self.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: TableSpec = TableSpec {
        name: "t",
        required: &["key"],
        optional: &["name"],
    };

    fn table_of(text: &str, findings: &mut Findings) -> Table {
        let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
        let csv = crate::csv::parse(&bytes, &crate::csv::TableLimits::default()).unwrap();
        Table::new(SPEC, "t.csv", bytes, csv, ImportMode::Strict, findings)
            .expect("the required column is there")
    }

    #[test]
    fn a_key_that_is_not_nfc_drops_its_row() {
        let mut findings = Findings::default();
        // Row 2 has a decomposed e-acute key ("e" + U+0301), rows 1 and 3 are fine.
        let t = table_of("key,name\nA,x\ne\u{301},y\nB,z\n", &mut findings);
        let keys: Vec<(&str, u32)> = t.rows().map(|r| (r.key(), r.line())).collect();
        assert_eq!(keys, [("A", 2), ("B", 4)]);
        assert_eq!(t.len(), 2);
        let entries = findings.finish();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.code, Code::MALFORMED_ITEM);
        assert_eq!(e.severity, Severity::Error);
        assert_eq!(
            e.message,
            "row dropped: the key is not in Unicode NFC (keys are hashed byte for byte)"
        );
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.file, "t.csv");
        assert_eq!(loc.item.as_deref(), Some("e\u{301}"));
        assert_eq!(loc.line, Some(3));
    }

    #[test]
    fn an_nfc_key_is_kept_byte_for_byte() {
        let mut findings = Findings::default();
        let t = table_of("key,name\n\u{e9},x\n", &mut findings);
        assert_eq!(t.rows().map(|r| r.key()).collect::<Vec<_>>(), ["\u{e9}"]);
        assert!(findings.finish().is_empty());
    }

    #[test]
    fn a_cell_that_is_not_nfc_is_normalised_and_an_nfc_cell_is_not_copied() {
        let mut findings = Findings::default();
        let t = table_of("key,name\nA,e\u{301}\nB,\u{e9}\nC,plain\n", &mut findings);
        let names: Vec<&str> = t.rows().map(|r| r.get("name").unwrap()).collect();
        assert_eq!(names, ["\u{e9}", "\u{e9}", "plain"]);
        assert!(findings.finish().is_empty());
        // Only the decomposed cell went to the side buffer.
        assert_eq!(t.owned, "\u{e9}");
    }

    #[test]
    fn dropped_rows_do_not_shift_the_cells_of_the_rows_after_them() {
        let mut findings = Findings::default();
        let t = table_of(
            "key,name\ne\u{301},a\nB,b\ne\u{301}2,c\nC,\"d\"\"d\"\n",
            &mut findings,
        );
        let rows: Vec<(&str, &str, u32)> = t
            .rows()
            .map(|r| (r.key(), r.get("name").unwrap(), r.line()))
            .collect();
        assert_eq!(rows, [("B", "b", 3), ("C", "d\"d", 5)]);
    }

    fn colour(s: &str, mode: ImportMode) -> Result<ColourSpec, String> {
        parse_colour(s, mode).map(|(spec, _)| spec)
    }

    #[test]
    fn colours() {
        for mode in [ImportMode::Strict, ImportMode::Lenient] {
            assert_eq!(
                colour("BK", mode).unwrap().codes(),
                [ColourCode::Iec(IecColour::Bk)]
            );
            assert_eq!(
                colour("GNYE", mode).unwrap().codes(),
                [ColourCode::Iec(IecColour::Gn), ColourCode::Iec(IecColour::Ye)]
            );
            // A company code is no foreign code in either mode, and neither is a cell that mixes in a
            // letter pair that is no alias.
            assert!(matches!(
                parse_colour("DBU", mode).unwrap(),
                (spec, None) if matches!(spec.codes(), [ColourCode::Company(_)])
            ));
            assert!(matches!(
                parse_colour("BRXX", mode).unwrap(),
                (spec, None) if matches!(spec.codes(), [ColourCode::Company(_)])
            ));
        }
    }

    #[test]
    fn a_foreign_colour_is_refused_strict_and_kept_lenient() {
        for (alias, iec) in [
            ("BR", IecColour::Bn),
            ("SW", IecColour::Bk),
            ("BL", IecColour::Bu),
            ("RT", IecColour::Rd),
            ("GE", IecColour::Ye),
            ("WS", IecColour::Wh),
            ("GR", IecColour::Gy),
        ] {
            assert_eq!(
                colour(alias, ImportMode::Strict).unwrap_err(),
                format!(
                    "`{alias}` is a foreign colour code (IEC 60757: {}); a strict import stores no foreign code (EW-CORE-031)",
                    iec.as_str()
                )
            );
            let (spec, found) = parse_colour(alias, ImportMode::Lenient).unwrap();
            assert_eq!(spec.codes(), [ColourCode::Foreign(alias.parse().unwrap())]);
            let found = found.unwrap();
            assert_eq!(found.aliases, [(alias, iec)]);
            assert_eq!(found.mapped.codes(), [ColourCode::Iec(iec)]);
        }
    }

    #[test]
    fn a_two_chunk_cell_keeps_its_iec_chunk() {
        let (spec, found) = parse_colour("BRWH", ImportMode::Lenient).unwrap();
        assert_eq!(
            spec.codes(),
            [
                ColourCode::Foreign("BR".parse().unwrap()),
                ColourCode::Iec(IecColour::Wh)
            ]
        );
        let found = found.unwrap();
        assert_eq!(found.aliases, [("BR", IecColour::Bn)]);
        assert_eq!(
            found.mapped.codes(),
            [ColourCode::Iec(IecColour::Bn), ColourCode::Iec(IecColour::Wh)]
        );
        let err = colour("BRWH", ImportMode::Strict).unwrap_err();
        assert!(
            err.starts_with("`BR` is a foreign colour code (IEC 60757: BN);"),
            "{err}"
        );
        // Two aliases list each pair.
        let err = colour("BRWS", ImportMode::Strict).unwrap_err();
        assert_eq!(
            err,
            "`BR` → `BN` and `WS` → `WH` are foreign colour codes (IEC 60757); a strict import stores no foreign code (EW-CORE-031)"
        );
        // Three chunks are no colour spec in either mode.
        assert!(colour("BRWHBK", ImportMode::Lenient).is_err());
    }

    #[test]
    fn integers() {
        assert_eq!(parse_uint::<u32>("12"), Ok(12));
        assert!(parse_uint::<u8>("300").is_err());
        assert!(parse_uint::<u32>("-1").is_err());
        assert!(parse_uint::<u32>("1.0").is_err());
    }
}
