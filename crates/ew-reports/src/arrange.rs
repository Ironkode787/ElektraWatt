// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filtering, manual selection, sorting and grouping of report rows (doc 09 §3.3, FR-REP-027,
//! FR-I18N-010).
//!
//! [`arrange`] runs, in order: manual exclusion, filter, sort within heading segments, grouping with
//! optional header and footer rows. Sorting compares [`cell_key`]s, a total order over typed cells that
//! never reads the page order (pagination runs inside it), with the [`RowId`] as the last tie-break, so the
//! result does not depend on the order the kind produced the rows in. Group titles and filters use the
//! source-language machine text of [`cell_text`]; display formatting is the consumer's.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::{MlText, NaturalKey, natural_key};
use ew_derive::ew_schema::{AnyId, ContinuationPolicy, Date, Device, GroupSpec, ReportBlock, Set, SortKey};
use ew_derive::ew_units::Dec3;
use ew_derive::{DeviceTag, PageRef, Q, ResolvedStructure, SectionStructure, folio_label};

use crate::BlockPages;
use crate::layout::PageLayout;
use crate::paginate::is_toc;
use crate::rowset::{
    Cell, CellType, ColumnDef, GroupRange, MissingReason, QtyUnit, Row, RowId, RowKey, RowKind, RowSet,
};

/// A typed row predicate (doc 09 §3.3). The persisted encoding of a block's `filter` member is not
/// decided yet (doc 09 §2.2: deferred, kept in `extra`), so blocks do not carry one; the engine evaluates
/// filters handed to it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Filter {
    /// Every predicate holds (true when empty).
    All(Vec<Filter>),
    /// At least one predicate holds (false when empty).
    Any(Vec<Filter>),
    /// The predicate does not hold.
    Not(Box<Filter>),
    /// A predicate over one column's cell.
    Column {
        /// The column key; an unknown key makes the predicate false.
        key: String,
        /// The test.
        op: FilterOp,
    },
}

/// A test on one cell, over its text in the project's source language.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FilterOp {
    /// The text equals the value.
    Eq(String),
    /// The text differs from the value.
    Ne(String),
    /// The text equals the value or starts with it followed by a character that is not a letter or
    /// digit: `location within +J-E-2416` holds for `+J-E-2416` and `+J-E-2416.1`, not `+J-E-24160`.
    Within(String),
    /// The cell has a value (not `Empty`, `Tbd` or `Missing`).
    Present,
    /// The cell has no value (`Empty`, `Tbd` or `Missing`).
    Absent,
}

/// How a block arranges its rows.
#[derive(Clone, Debug)]
pub struct Arrangement<'a> {
    /// The sort keys (column key, descending).
    pub sort: &'a [SortKey],
    /// The grouping.
    pub group: Option<&'a GroupSpec>,
    /// The filter.
    pub filter: Option<&'a Filter>,
    /// Entities whose rows are always included.
    pub include: &'a Set<AnyId>,
    /// Entities whose rows are excluded.
    pub exclude: &'a Set<AnyId>,
    /// Whether to emit group-header rows (the form has a `group_header` template).
    pub group_header: bool,
    /// Whether to emit group-footer rows (the form has a `group_footer` template).
    pub group_footer: bool,
    /// The structure the generated pages print: the block's section structure, absent without a section or
    /// when it is empty. Group texts print a tag in its displayed form for this context (else its short
    /// form); the grouping order keeps using the full tag.
    pub group_context: Option<Arc<ResolvedStructure>>,
}

impl<'a> Arrangement<'a> {
    /// The arrangement of a block whose first page has `layout`; no filter (see [`Filter`]).
    pub fn of_block(block: &'a ReportBlock, layout: &PageLayout) -> Arrangement<'a> {
        Arrangement {
            sort: &block.sort,
            group: block.group.as_ref(),
            filter: None,
            include: &block.manual.include,
            exclude: &block.manual.exclude,
            group_header: layout.group_header.is_some(),
            group_footer: layout.group_footer.is_some(),
            group_context: None,
        }
    }

    /// Sets the group-text context from the block's section structure (see [`Arrangement::group_context`]).
    #[must_use]
    pub fn with_group_context(mut self, q: &Q, block: &ReportBlock) -> Arrangement<'a> {
        self.group_context = block
            .section
            .map(|s| q.get::<SectionStructure>(s))
            .filter(|st| **st != ResolvedStructure::default());
        self
    }
}

/// Applies an arrangement to a row set in the kind's default order: manual exclusion, filter, sort within
/// heading segments, grouping (see the module documentation). A set that is not derived passes through
/// unchanged.
pub fn arrange(q: &Q, set: RowSet, how: &Arrangement<'_>) -> RowSet {
    if !set.is_derived() {
        return set;
    }
    let RowSet {
        columns,
        rows,
        groups: _,
        as_of,
        status,
    } = set;

    // Steps 1 and 2: manual exclusion and filter, per segment.
    let mut segments = split_segments(rows);
    for seg in &mut segments {
        seg.rows.retain(|r| keep_row(q, &columns, r, how));
        if seg.heading.is_some() && !seg.rows.iter().any(|r| r.kind == RowKind::Data) {
            seg.heading = None;
        }
    }
    segments.retain(|s| s.heading.is_some() || !s.rows.is_empty());

    // Step 3: sort within each segment. Sort keys that name no column are skipped; when none names one, the
    // kind's order is kept (as with no sort keys at all).
    if how
        .sort
        .iter()
        .any(|k| columns.iter().any(|c| c.key == k.key.as_str()))
    {
        let keys: Vec<(usize, bool)> = how
            .sort
            .iter()
            .filter_map(|k| {
                columns
                    .iter()
                    .position(|c| c.key == k.key.as_str())
                    .map(|i| (i, k.descending))
            })
            .collect();
        for seg in &mut segments {
            sort_rows(q, &mut seg.rows, &keys);
        }
    }

    // Step 4: grouping.
    let group_col = how
        .group
        .and_then(|g| columns.iter().position(|c| c.key == g.key.as_str()));
    let (rows, groups) = match group_col {
        Some(col) => group_rows(q, &columns, segments, col, how),
        None => (flatten(segments), Vec::new()),
    };
    RowSet {
        columns,
        rows,
        groups,
        as_of,
        status,
    }
}

/// A heading with the rows up to the next heading.
struct Segment {
    heading: Option<Row>,
    rows: Vec<Row>,
}

fn split_segments(rows: Vec<Row>) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    for row in rows {
        if row.kind == RowKind::Heading {
            out.push(Segment {
                heading: Some(row),
                rows: Vec::new(),
            });
        } else {
            match out.last_mut() {
                Some(seg) => seg.rows.push(row),
                None => out.push(Segment {
                    heading: None,
                    rows: vec![row],
                }),
            }
        }
    }
    out
}

fn flatten(segments: Vec<Segment>) -> Vec<Row> {
    let mut out = Vec::new();
    for seg in segments {
        out.extend(seg.heading);
        out.extend(seg.rows);
    }
    out
}

/// The entities a data row covers: its sources, else the entity components of its ID.
fn row_ids(row: &Row) -> Vec<AnyId> {
    if row.sources.is_empty() {
        row.id.entities().map(AnyId::from_uuid).collect()
    } else {
        row.sources.clone()
    }
}

/// Manual exclusion and filter for one row (headings and group rows are kept here).
fn keep_row(q: &Q, columns: &[ColumnDef], row: &Row, how: &Arrangement<'_>) -> bool {
    if row.kind != RowKind::Data {
        return true;
    }
    let ids = row_ids(row);
    let included = ids.iter().any(|i| how.include.contains(i));
    if !ids.is_empty() && !included && ids.iter().all(|i| how.exclude.contains(i)) {
        return false;
    }
    match how.filter {
        Some(f) => included || filter_holds(q, columns, row, f),
        None => true,
    }
}

fn filter_holds(q: &Q, columns: &[ColumnDef], row: &Row, f: &Filter) -> bool {
    match f {
        Filter::All(fs) => fs.iter().all(|f| filter_holds(q, columns, row, f)),
        Filter::Any(fs) => fs.iter().any(|f| filter_holds(q, columns, row, f)),
        Filter::Not(f) => !filter_holds(q, columns, row, f),
        Filter::Column { key, op } => {
            let Some(i) = columns.iter().position(|c| c.key == *key) else {
                return false;
            };
            let cell = row.cells.get(i).unwrap_or(&Cell::Empty);
            match op {
                FilterOp::Eq(v) => cell_text(q, cell) == *v,
                FilterOp::Ne(v) => cell_text(q, cell) != *v,
                FilterOp::Within(v) => within(&cell_text(q, cell), v),
                FilterOp::Present => has_value(cell),
                FilterOp::Absent => !has_value(cell),
            }
        }
    }
}

fn within(text: &str, prefix: &str) -> bool {
    match text.strip_prefix(prefix) {
        Some(rest) => rest.chars().next().is_none_or(|c| !c.is_alphanumeric()),
        None => false,
    }
}

fn has_value(cell: &Cell) -> bool {
    match cell {
        Cell::Empty | Cell::Tbd | Cell::Missing(_) => false,
        Cell::Stale(inner, _) => has_value(inner),
        _ => true,
    }
}

/// Sorts `rows` by the column keys `(index, descending)`, ties by `RowId`.
fn sort_rows(q: &Q, rows: &mut Vec<Row>, keys: &[(usize, bool)]) {
    let mut keyed: Vec<(Vec<CellKey>, Row)> = std::mem::take(rows)
        .into_iter()
        .map(|r| {
            let k = keys
                .iter()
                .map(|(i, _)| cell_key(q, r.cells.get(*i).unwrap_or(&Cell::Empty)))
                .collect();
            (k, r)
        })
        .collect();
    keyed.sort_by(|(ka, a), (kb, b)| {
        for (n, (_, descending)) in keys.iter().enumerate() {
            let o = ka[n].cmp(&kb[n]);
            let o = if *descending { o.reverse() } else { o };
            if o != Ordering::Equal {
                return o;
            }
        }
        a.id.cmp(&b.id)
    });
    rows.extend(keyed.into_iter().map(|(_, r)| r));
}

/// One group being collected: the key cell of its first row and its rows.
struct Collected {
    key: Cell,
    rows: Vec<Row>,
}

fn group_rows(
    q: &Q,
    columns: &[ColumnDef],
    segments: Vec<Segment>,
    col: usize,
    how: &Arrangement<'_>,
) -> (Vec<Row>, Vec<GroupRange>) {
    // Groups follow the sort direction of the group column; a heading bounds its groups.
    let descending = how
        .group
        .is_some_and(|g| how.sort.iter().any(|k| k.key == g.key && k.descending));
    let mut rows: Vec<Row> = Vec::new();
    let mut groups: Vec<GroupRange> = Vec::new();
    for seg in segments {
        let mut collected: BTreeMap<CellKey, Collected> = BTreeMap::new();
        let mut put = |row: Row, key: &Cell| {
            collected
                .entry(cell_key(q, key))
                .or_insert_with(|| Collected {
                    key: key.clone(),
                    rows: Vec::new(),
                })
                .rows
                .push(row);
        };
        let key_of = |r: &Row| r.cells.get(col).unwrap_or(&Cell::Empty).clone();
        // The heading takes the first data row's group; a non-data row inside the segment takes the
        // next data row's, else the last one's.
        let first_data = seg.rows.iter().find(|r| r.kind == RowKind::Data).map(key_of);
        if let Some(h) = seg.heading {
            put(h, &first_data.clone().unwrap_or(Cell::Empty));
        }
        let mut pending: Vec<Row> = Vec::new();
        let mut last = first_data;
        for row in seg.rows {
            if row.kind == RowKind::Data {
                let key = key_of(&row);
                for p in pending.drain(..) {
                    put(p, &key);
                }
                put(row, &key);
                last = Some(key);
            } else {
                pending.push(row);
            }
        }
        let key = last.unwrap_or(Cell::Empty);
        for p in pending {
            put(p, &key);
        }

        let ordered: Vec<Collected> = if descending {
            collected.into_values().rev().collect()
        } else {
            collected.into_values().collect()
        };
        for g in ordered {
            let start = len_u32(rows.len());
            let text = group_text(q, how, &g.key);
            if how.group_header {
                let header = group_row(columns.len(), col, RowKind::GroupHeader, &g.key, &text);
                rows.push(header);
            }
            let footer = how.group_footer.then(|| {
                let mut row = group_row(columns.len(), col, RowKind::GroupFooter, &g.key, &text);
                row.cells = footer_cells(columns, col, &g);
                row
            });
            rows.extend(g.rows);
            rows.extend(footer);
            groups.push(GroupRange {
                text,
                key: g.key,
                rows: start..len_u32(rows.len()),
            });
        }
    }
    (rows, groups)
}

/// The text of a group key: a tag prints in its displayed form for the generated pages' structure
/// (`how.group_context`), else its short form; other cells print as [`cell_text`].
fn group_text(q: &Q, how: &Arrangement<'_>, key: &Cell) -> String {
    let tag = |id: Id<Device>| {
        let tag = q.get::<DeviceTag>(id);
        match (*tag).as_ref() {
            Some(t) => match &how.group_context {
                Some(ctx) => t.displayed(ctx),
                None => t.short(),
            },
            None => String::new(),
        }
    };
    match peel(key) {
        Cell::Tag(id) => tag(*id),
        Cell::Range(ids) => ids.iter().map(|id| tag(*id)).collect::<Vec<_>>().join(", "),
        other => cell_text(q, other),
    }
}

/// A group header or footer row: empty cells except the key column.
fn group_row(width: usize, col: usize, kind: RowKind, key: &Cell, text: &str) -> Row {
    let mut cells = vec![Cell::Empty; width];
    cells[col] = key.clone();
    let k = match peel(key) {
        Cell::Tag(id) => RowKey::Entity(id.as_uuid()),
        Cell::Folio(PageRef::Drawn(id)) => RowKey::Entity(id.as_uuid()),
        _ => RowKey::Name(text.to_owned()),
    };
    let name = if kind == RowKind::GroupHeader {
        "group-header"
    } else {
        "group-footer"
    };
    Row {
        id: RowId(vec![RowKey::Name(name.to_owned()), k]),
        kind,
        cells,
        links: Vec::new(),
        sources: Vec::new(),
    }
}

/// The cells of a group footer: the key, and the sum of every `Qty` column.
fn footer_cells(columns: &[ColumnDef], col: usize, g: &Collected) -> Vec<Cell> {
    columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == col {
                g.key.clone()
            } else if c.ty == CellType::Qty {
                sum_qty(
                    g.rows
                        .iter()
                        .filter(|r| r.kind == RowKind::Data)
                        .map(|r| r.cells.get(i)),
                )
            } else {
                Cell::Empty
            }
        })
        .collect()
}

/// The sum of the `Qty` cells when they share one unit, else `Empty`.
fn sum_qty<'a>(cells: impl Iterator<Item = Option<&'a Cell>>) -> Cell {
    let mut total: Option<(Dec3, &QtyUnit)> = None;
    for cell in cells.flatten() {
        let Cell::Qty(v, u) = peel(cell) else { continue };
        match &mut total {
            None => total = Some((*v, u)),
            Some((sum, unit)) if *unit == u => *sum = Dec3(sum.0.saturating_add(v.0)),
            Some(_) => return Cell::Empty,
        }
    }
    match total {
        Some((sum, unit)) => Cell::Qty(sum, unit.clone()),
        None => Cell::Empty,
    }
}

/// The cell under any `Stale` wrappers.
fn peel(cell: &Cell) -> &Cell {
    match cell {
        Cell::Stale(inner, _) => peel(inner),
        c => c,
    }
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The sort key of a cell: a total order over cells of any type (see [`cell_key`]).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CellKey(Class);

/// Real values, then `Tbd`, then `Missing`, then `Empty`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Class {
    Value(Val),
    Tbd,
    Missing(MissingReason),
    Empty,
}

/// A value by cell variant (the variant order is the rank among values), then within the variant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Val {
    Text(NaturalKey),
    Ident(NaturalKey),
    Number(i64),
    Tag(TagKey),
    Folio(FolioKey),
    Folios(Vec<FolioKey>),
    CellRef(FolioKey, u32, u32),
    Qty(Dec3),
    Colour(NaturalKey),
    Address(NaturalKey),
    Date(Date),
    Bool(bool),
    Range(Vec<TagKey>),
}

/// The full tag text in natural order, then the device (the tie-break of equal tags).
type TagKey = (NaturalKey, Id<Device>);

/// Folio number, label in natural order, index within a generated block.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct FolioKey(u32, NaturalKey, u32);

/// The total order of sorting and grouping: real values by cell variant and then by value (text in natural
/// order, so "X2" < "X10"; tags by their full tag text; folios by number), then `Tbd`, then `Missing`,
/// then `Empty`. A `Stale` cell is keyed by its inner cell. Never reads the page order.
pub fn cell_key(q: &Q, cell: &Cell) -> CellKey {
    CellKey(match cell {
        Cell::Text(t) => Class::Value(Val::Text(natural_key(&source_text(q, t)))),
        Cell::Ident(s) => Class::Value(Val::Ident(natural_key(s))),
        Cell::Number(n) => Class::Value(Val::Number(*n)),
        Cell::Tag(id) => Class::Value(Val::Tag(tag_key(q, *id))),
        Cell::Folio(p) => Class::Value(Val::Folio(folio_key(q, *p))),
        Cell::Folios(ps) => Class::Value(Val::Folios(ps.iter().map(|p| folio_key(q, *p)).collect())),
        Cell::CellRef(p, c) => Class::Value(Val::CellRef(folio_key(q, *p), c.column, c.row)),
        Cell::Qty(v, _) => Class::Value(Val::Qty(*v)),
        Cell::Colour(c) => Class::Value(Val::Colour(natural_key(&crate::render::colour_text(c)))),
        Cell::Address(s) => Class::Value(Val::Address(natural_key(s))),
        Cell::Date(d) => Class::Value(Val::Date(*d)),
        Cell::Bool(b) => Class::Value(Val::Bool(*b)),
        Cell::Range(ids) => Class::Value(Val::Range(ids.iter().map(|id| tag_key(q, *id)).collect())),
        Cell::Tbd => Class::Tbd,
        Cell::Missing(r) => Class::Missing(*r),
        Cell::Stale(inner, _) => return cell_key(q, inner),
        Cell::Empty => Class::Empty,
    })
}

/// The machine text of a cell in the project's source language: group titles and filters. Display
/// rendering (number format, tag form, date format) is the consumer's, not this function's.
pub fn cell_text(q: &Q, cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => source_text(q, t),
        Cell::Ident(s) | Cell::Address(s) => s.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Tag(id) => tag_text(q, *id),
        Cell::Folio(p) => folio_text(q, *p),
        Cell::Folios(ps) => ps
            .iter()
            .map(|p| folio_text(q, *p))
            .collect::<Vec<_>>()
            .join(", "),
        Cell::CellRef(p, c) => format!("{}-{c}", folio_text(q, *p)),
        Cell::Qty(v, _) => v.to_string(),
        Cell::Colour(c) => crate::render::colour_text(c),
        Cell::Date(d) => d.to_string(),
        Cell::Bool(b) => b.to_string(),
        Cell::Range(ids) => ids
            .iter()
            .map(|id| tag_text(q, *id))
            .collect::<Vec<_>>()
            .join(", "),
        Cell::Tbd => "TBD".to_owned(),
        Cell::Missing(_) | Cell::Empty => String::new(),
        Cell::Stale(inner, _) => cell_text(q, inner),
    }
}

/// The text of a multi-language value in the project's source language; else its neutral text, else the
/// first language's (in tag order), else empty.
fn source_text(q: &Q, t: &MlText) -> String {
    if let Some(n) = t.as_neutral() {
        return n.to_owned();
    }
    if let Some(langs) = &q.project().languages
        && let Some(s) = t.get(&langs.source)
    {
        return s.to_owned();
    }
    t.languages()
        .next()
        .and_then(|l| t.get(l))
        .unwrap_or_default()
        .to_owned()
}

fn tag_text(q: &Q, id: Id<Device>) -> String {
    let tag = q.get::<DeviceTag>(id);
    (*tag).as_ref().map(|t| t.full.clone()).unwrap_or_default()
}

fn tag_key(q: &Q, id: Id<Device>) -> TagKey {
    (natural_key(&tag_text(q, id)), id)
}

/// The folio key of a page without reading the page order (this runs inside pagination).
fn folio_key(q: &Q, p: PageRef) -> FolioKey {
    match p {
        PageRef::Drawn(id) => match q.pages().get(id) {
            Some(page) => FolioKey(page.folio.number, natural_key(&folio_label(page)), 0),
            None => FolioKey(u32::MAX, natural_key(""), 0),
        },
        PageRef::Divider(id) => match q.sections().get(id) {
            Some(s) => FolioKey(s.folio_range.0, natural_key(&s.folio_range.0.to_string()), 0),
            None => FolioKey(u32::MAX, natural_key(""), 0),
        },
        PageRef::Generated { block, index } => generated_folio(q, block, index).0,
    }
}

/// The folio label of a page without reading the page order.
fn folio_text(q: &Q, p: PageRef) -> String {
    match p {
        PageRef::Drawn(id) => q
            .pages()
            .get(id)
            .map(|page| folio_label(page))
            .unwrap_or_default(),
        PageRef::Divider(id) => q
            .sections()
            .get(id)
            .map(|s| s.folio_range.0.to_string())
            .unwrap_or_default(),
        PageRef::Generated { block, index } => generated_folio(q, block, index).1,
    }
}

/// The folio key and label of page `index` of a block's generated pages.
///
/// A block other than a table of contents answers from its own pages (`BlockPages`: the actual number and
/// label, which differ from `first.index` under `NextFolio` and after a `NewFolio` group break); when that
/// query is running (the block arranging its own rows, or a cycle) or has no such page, the label is
/// computed from the first folio and the sub-page index. A table of contents never reads its pages (their
/// count is the page-order fixpoint's): its label follows the block's continuation policy.
fn generated_folio(q: &Q, block: Id<ReportBlock>, index: u32) -> (FolioKey, String) {
    let Some(b) = q.report_blocks().get(block) else {
        return (FolioKey(u32::MAX, natural_key(""), index), String::new());
    };
    let first = b.first_folio.unwrap_or(0);
    let sub_page = || {
        let label = if index == 0 {
            first.to_string()
        } else {
            format!("{first}.{index}")
        };
        (FolioKey(first, natural_key(""), index), label)
    };
    let numbered = |number: u32, label: String| (FolioKey(number, natural_key(&label), 0), label);
    if is_toc(b) {
        return match b.continuation {
            ContinuationPolicy::NextFolio => {
                let number = first.saturating_add(index);
                numbered(number, number.to_string())
            }
            _ => sub_page(),
        };
    }
    match q.try_get::<BlockPages>((block, 0)) {
        Ok(pages) => match pages.iter().find(|g| g.index == index) {
            Some(g) => numbered(g.number, g.label.clone()),
            None => sub_page(),
        },
        Err(cycle) => {
            q.note_cycle(cycle);
            sub_page()
        }
    }
}
