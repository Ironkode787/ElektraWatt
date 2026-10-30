// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `RowSet`: one row source for every consumer (doc 09 §2.3, ADR-0018).
//!
//! A row set holds **typed cells**: values, never strings. The display language, number format, tag form
//! and date format are applied by the consumer (late formatting, FR-I18N-009), so XLSX gets numbers as
//! numbers while a PDF prints "0,75 mm²" in an nl-NL export.
//!
//! Every row has a **stable [`RowId`]** derived from its source entity IDs (FR-REP-048): a device ID for a
//! device-list row, `(part, variant, mode)` for a BOM row, `(strip, terminal, level)` for a terminal row,
//! the target-side connection for a wire row. Tags are never row keys, so renumbering keeps row identity;
//! the ID keys back-links (FR-REP-042), the hidden `id` column of the Excel round trip (FR-EXP-004) and
//! revision deltas (FR-REV-015).

use std::fmt::{self, Write as _};
use std::ops::Range;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{AnyId, ColourSpec, Date, Device, Function, OrderUnit, Unit};
use ew_derive::ew_units::Dec3;
use ew_derive::{PageRef, Rev};

/// The frame-grid cell of a cross-reference target (`H16`), as `ew-derive` computes it.
pub type GridCell = ew_derive::Cell;

/// The rows of one report, in final order after filter, sort and grouping (doc 09 §2.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowSet {
    /// The column schema.
    pub columns: Vec<ColumnDef>,
    /// The rows: data, headings and, after grouping, group headers and footers.
    pub rows: Vec<Row>,
    /// The groups in order, each with its row range (header and footer included). Empty when the block
    /// does not group; pagination then treats the whole set as one group without a key.
    pub groups: Vec<GroupRange>,
    /// The revision the rows were computed at: `None` for Class I row sources, which are always fresh (the
    /// current revision; a revision in a memoised value would defeat early cut-off); `Some(rev)` for a
    /// Class II source such as the SQL projection (doc 09 §3.5).
    pub as_of: Option<Rev>,
    /// Whether the row source exists yet.
    pub status: RowSetStatus,
}

impl RowSet {
    /// An empty, derived row set with these columns.
    pub fn new(columns: Vec<ColumnDef>) -> RowSet {
        RowSet {
            columns,
            rows: Vec::new(),
            groups: Vec::new(),
            as_of: None,
            status: RowSetStatus::Derived,
        }
    }

    /// An empty row set whose source is not derived yet: the report has its columns but no rows, and the
    /// block paginates to one placeholder page.
    pub fn not_derived(columns: Vec<ColumnDef>, reason: NotDerived) -> RowSet {
        RowSet {
            status: RowSetStatus::NotDerived(reason),
            ..RowSet::new(columns)
        }
    }

    /// The position of the column with this key.
    pub fn column(&self, key: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.key == key)
    }

    /// The number of data rows (headings, group headers and footers excluded).
    pub fn data_rows(&self) -> usize {
        self.rows.iter().filter(|r| r.kind == RowKind::Data).count()
    }

    /// Whether the row source is derived.
    pub fn is_derived(&self) -> bool {
        self.status == RowSetStatus::Derived
    }
}

/// One group of an arranged row set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GroupRange {
    /// The group key as text in the project's source language (`{group}`, `{group.key}`).
    pub text: String,
    /// The group key cell.
    pub key: Cell,
    /// The rows of the group in [`RowSet::rows`].
    pub rows: Range<u32>,
}

/// Whether a row source exists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RowSetStatus {
    /// The rows are derived.
    Derived,
    /// The rows are not derived yet; the set is empty.
    NotDerived(NotDerived),
}

/// Why a report has no row source yet.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NotDerived {
    /// The row query of `ew-derive` it reads does not exist yet (the connectivity queries of Derivation II:
    /// `terminal_rows`, `cable_rows`, `wire_rows`, `potential_partition`, `plc_rows`; the revision overview's
    /// `revision_rows` until FT-207).
    AwaitsQuery(&'static str),
    /// The model has no record shape for the source data yet.
    NoRecordShape(&'static str),
    /// The source is not supported in this build: a report kind without a row query, a user SQL report
    /// before 1.0, or a kind of a newer format.
    Unsupported(String),
    /// A query cycle: the row source read the page order (or the block's own pages) from inside the
    /// pagination (ADR-0002 decision 4); the text is the family that was already evaluating.
    Cycle(String),
}

impl fmt::Display for NotDerived {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotDerived::AwaitsQuery(q) => write!(f, "not derived yet: awaits the `{q}` query"),
            NotDerived::NoRecordShape(r) => write!(f, "not derived yet: no `{r}` records in this format"),
            NotDerived::Unsupported(s) => write!(f, "not derived: `{s}` is not supported in this build"),
            NotDerived::Cycle(s) => write!(f, "not derived: query cycle in `{s}`"),
        }
    }
}

/// One column of a report.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ColumnDef {
    /// The key: `{row.<key>}` in forms, the header of CSV and XLSX, sort and group keys.
    pub key: String,
    /// The cell type every data cell of the column has (or one of the absent values).
    pub ty: CellType,
    /// The header label (English; Fluent keys arrive with the message-list UI).
    pub label: MlText,
    /// The relative printed width of the column in a paged table: the house report form splits its table
    /// width by these weights ([`ColumnDef::new`] gives [`ColumnDef::DEFAULT_WEIGHT`]). Not data: XLSX and CSV
    /// ignore it.
    pub weight: u32,
}

impl ColumnDef {
    /// The weight of a column that does not set one.
    pub const DEFAULT_WEIGHT: u32 = 10;

    /// A column with a neutral-text label.
    pub fn new(key: &str, ty: CellType, label: &str) -> ColumnDef {
        ColumnDef {
            key: key.to_owned(),
            ty,
            label: MlText::neutral(label),
            weight: ColumnDef::DEFAULT_WEIGHT,
        }
    }

    /// This column with printed-width weight `weight`.
    #[must_use]
    pub fn weighted(mut self, weight: u32) -> ColumnDef {
        self.weight = weight;
        self
    }
}

/// The value type of a column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CellType {
    /// [`Cell::Text`].
    Text,
    /// [`Cell::Ident`].
    Ident,
    /// [`Cell::Number`].
    Number,
    /// [`Cell::Tag`].
    Tag,
    /// [`Cell::Folio`].
    Folio,
    /// [`Cell::Folios`].
    Folios,
    /// [`Cell::CellRef`].
    CellRef,
    /// [`Cell::Qty`].
    Qty,
    /// [`Cell::Colour`].
    Colour,
    /// [`Cell::Address`].
    Address,
    /// [`Cell::Date`].
    Date,
    /// [`Cell::Bool`].
    Bool,
    /// [`Cell::Range`].
    Range,
}

/// One row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Row {
    /// The stable identity.
    pub id: RowId,
    /// The row kind; each has its own row template in the form.
    pub kind: RowKind,
    /// One cell per column, in column order.
    pub cells: Vec<Cell>,
    /// The link destinations of cells.
    pub links: Vec<LinkRef>,
    /// The entities the row covers (the devices of a BOM row); manual exclusion and back-links read them.
    pub sources: Vec<AnyId>,
}

impl Row {
    /// A data row.
    pub fn data(id: RowId, cells: Vec<Cell>) -> Row {
        Row {
            id,
            kind: RowKind::Data,
            cells,
            links: Vec::new(),
            sources: Vec::new(),
        }
    }
}

/// The kind of a row (doc 09 §2.3); a form's data area has one template per kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum RowKind {
    /// A data row.
    Data,
    /// A group header (emitted only when the form has a `group_header` template).
    GroupHeader,
    /// A group footer with sums (emitted only when the form has a `group_footer` template).
    GroupFooter,
    /// A heading inside the rows (the cable headings of folio 906).
    Heading,
}

/// The stable identity of a row: the row's source entity IDs and discriminators (FR-REP-048).
///
/// Ordered component by component, which gives the deterministic tie-break of sorting.
///
/// # Text form
///
/// [`Display`](fmt::Display) prints the components joined by `/`, each typed and escaped: `e:<uuid>` for an
/// entity (lowercase, hyphenated), `i:<decimal>` for an index and `n:<name>` for a name, where every byte of
/// the name's UTF-8 that is not an ASCII letter, digit or one of `-_.~` is written `%XX` (uppercase hex).
/// So `/`, `:`, `%`, spaces and non-ASCII text never occur raw, and the form is **injective**: two different
/// IDs never print alike (`Name("a/b")` is `n:a%2Fb`, `[Name("a"), Name("b")]` is `n:a/n:b`). The empty ID
/// prints as the empty string. [`FromStr`](std::str::FromStr) reads exactly this form back.
///
/// This is the hidden `id` column of the Excel round trip (FR-EXP-004) and the key of revision deltas, so
/// the form is stable (FR-REP-048): a change to it breaks files written earlier.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RowId(pub Vec<RowKey>);

impl RowId {
    /// The ID of a row of exactly one entity.
    pub fn entity<T>(id: Id<T>) -> RowId {
        RowId(vec![RowKey::Entity(id.as_uuid())])
    }

    /// The ID with `key` appended.
    #[must_use]
    pub fn with(mut self, key: RowKey) -> RowId {
        self.0.push(key);
        self
    }

    /// The entity components.
    pub fn entities(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.0.iter().filter_map(|k| match k {
            RowKey::Entity(u) => Some(*u),
            _ => None,
        })
    }
}

impl RowId {
    /// Parses the text form written by [`Display`](fmt::Display); see [`RowId`].
    ///
    /// # Errors
    ///
    /// [`RowIdParseError`] for an unknown component tag, an entity that is not a canonical (lowercase,
    /// hyphenated) UUID, an index that is not a canonical `u32`, a raw character that must be escaped, an
    /// invalid `%` escape or a name that is not UTF-8 after unescaping.
    pub fn parse(s: &str) -> Result<RowId, RowIdParseError> {
        if s.is_empty() {
            return Ok(RowId(Vec::new()));
        }
        s.split('/')
            .map(parse_component)
            .collect::<Result<Vec<_>, _>>()
            .map(RowId)
    }
}

/// The text form of a [`RowId`] (see there): typed, `/`-joined and percent-escaped, hence injective.
impl fmt::Display for RowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, k) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("/")?;
            }
            match k {
                RowKey::Entity(u) => write!(f, "e:{}", u.hyphenated())?,
                RowKey::Index(n) => write!(f, "i:{n}")?,
                RowKey::Name(s) => {
                    f.write_str("n:")?;
                    for &b in s.as_bytes() {
                        if is_unreserved(b) {
                            f.write_char(char::from(b))?;
                        } else {
                            write!(f, "%{b:02X}")?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl std::str::FromStr for RowId {
    type Err = RowIdParseError;

    fn from_str(s: &str) -> Result<RowId, RowIdParseError> {
        RowId::parse(s)
    }
}

/// Why a text is not a [`RowId`] (see [`RowId::parse`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RowIdParseError {
    message: String,
}

impl RowIdParseError {
    fn new(message: impl Into<String>) -> RowIdParseError {
        RowIdParseError {
            message: message.into(),
        }
    }

    /// What is wrong, in English.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RowIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid row ID: {}", self.message)
    }
}

impl std::error::Error for RowIdParseError {}

/// The bytes a name prints raw: ASCII letters and digits and `-_.~`.
fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')
}

fn parse_component(c: &str) -> Result<RowKey, RowIdParseError> {
    let Some((tag, body)) = c.split_once(':') else {
        return Err(RowIdParseError::new(format!("component `{c}` has no type tag")));
    };
    match tag {
        "e" => {
            let u: Uuid = body
                .parse()
                .map_err(|_| RowIdParseError::new(format!("`{body}` is not a UUID")))?;
            if u.hyphenated().to_string() != body {
                return Err(RowIdParseError::new(format!(
                    "`{body}` is not a lowercase hyphenated UUID"
                )));
            }
            Ok(RowKey::Entity(u))
        }
        "i" => {
            let n: u32 = body
                .parse()
                .map_err(|_| RowIdParseError::new(format!("`{body}` is not a u32")))?;
            if n.to_string() != body {
                return Err(RowIdParseError::new(format!(
                    "`{body}` is not a canonical decimal"
                )));
            }
            Ok(RowKey::Index(n))
        }
        "n" => unescape(body).map(RowKey::Name),
        other => Err(RowIdParseError::new(format!("unknown component tag `{other}`"))),
    }
}

fn unescape(s: &str) -> Result<String, RowIdParseError> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' {
            // Only the canonical form `%XX` with uppercase hex parses, so `parse(display(id)) == id` and no two
            // texts name one id.
            let hex = |k: usize| {
                bytes
                    .get(k)
                    .filter(|d| d.is_ascii_digit() || (b'A'..=b'F').contains(d))
                    .and_then(|d| char::from(*d).to_digit(16))
            };
            let (Some(hi), Some(lo)) = (hex(i + 1), hex(i + 2)) else {
                return Err(RowIdParseError::new(
                    "a `%` is not followed by two uppercase hex digits",
                ));
            };
            out.push(u8::try_from(hi * 16 + lo).unwrap_or(0));
            i += 3;
        } else if is_unreserved(b) {
            out.push(b);
            i += 1;
        } else {
            return Err(RowIdParseError::new(format!(
                "byte 0x{b:02X} in a name must be written as %{b:02X}"
            )));
        }
    }
    String::from_utf8(out).map_err(|_| RowIdParseError::new("a name is not valid UTF-8 after unescaping"))
}

/// One component of a [`RowId`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum RowKey {
    /// A source entity.
    Entity(Uuid),
    /// A position or level (a terminal level, a channel number).
    Index(u32),
    /// A literal discriminator (`"component"`, `"group-header"`).
    Name(String),
}

/// One typed cell (doc 09 §2.3). Formatting happens in the consumer ([`crate::render`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Cell {
    /// Multi-language text.
    Text(MlText),
    /// An identifier or code printed as is (part number, order code, potential name).
    Ident(String),
    /// An integer (a BOM position, a count).
    Number(i64),
    /// A device, printed in full or displayed form per the column setting.
    Tag(Id<Device>),
    /// A page of the page order (drawn, divider or generated), printed as its folio label.
    Folio(PageRef),
    /// Several pages, in page order (the "Folios" column of the BOM).
    Folios(Vec<PageRef>),
    /// A frame-grid cell on a page (`101-H16`).
    CellRef(PageRef, GridCell),
    /// An exact quantity with its unit.
    Qty(Dec3, QtyUnit),
    /// A colour: one code, or two (GNYE).
    Colour(ColourSpec),
    /// A PLC address as text (typed once `ew-plcaddr` has its address type).
    Address(String),
    /// A date.
    Date(Date),
    /// A flag.
    Bool(bool),
    /// A tag list, compressed at render time (doc 09 §3.6, FR-REP-046).
    Range(Vec<Id<Device>>),
    /// A value still to be decided ("TBD"; listed in open items, blocks release).
    Tbd,
    /// A value that cannot be derived.
    Missing(MissingReason),
    /// A Class II value computed at an older revision; printed with "≈", refused in release exports.
    Stale(Box<Cell>, Rev),
    /// No value.
    Empty,
}

/// The unit of a [`Cell::Qty`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum QtyUnit {
    /// A pure number.
    None,
    /// A physical unit (doc 03 §3.2).
    Unit(Unit),
    /// An order unit (piece, metre, set).
    Order(OrderUnit),
}

/// Why a [`Cell::Missing`] value is missing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MissingReason {
    /// A cable or wire length ("length missing", FR-REP-013).
    Length,
    /// A part assignment.
    Part,
    /// A cross-section.
    CrossSection,
    /// A colour.
    Colour,
}

/// The destination of one cell's link: the PDF turns it into a link annotation, the canvas into a jump.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkRef {
    /// The column of the cell.
    pub column: u16,
    /// The destination.
    pub target: LinkTarget,
}

/// A link destination.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LinkTarget {
    /// The main representation of a device.
    MainRep(Id<Device>),
    /// A connection point.
    Cp {
        /// The function.
        function: Id<Function>,
        /// The CP index.
        idx: u16,
    },
    /// A page.
    Page(PageRef),
}
