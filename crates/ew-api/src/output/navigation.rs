// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reads a viewer finds its way with (WP-S2.03 part 1, spec §4.2): the rows of the page navigator
//! ([`Session::page_rows`]), a page's frame cells ([`Session::page_cells`], [`PageCells`]), Go to page or cell
//! ([`Session::resolve_goto`]) and the targets of cross-reference links ([`Session::link_targets`]); and for the
//! canvas tools of part 2 the connection points drawn on a page ([`Session::page_cps`], [`PageCp`]).
//!
//! Each is a read over data the export path derives, through the session's memoising `Q`. [`PageCells`] carries
//! what `ew_derive`'s exact cell rule needs (the frame grid, the page's connection grid, the frame's
//! page-to-sheet map and the project's `ip` format), so a viewer evaluates the cell of a pointer position with no
//! `Session` call (NFR-PERF-20).
//!
//! **Sheet and page.** The cells are defined on the page; a viewer works on the sheet. A sheet point maps to the
//! page by the inverse of the frame's page-to-sheet map, `page = (sheet − offset) × 10⁹ / scale_ppb`, per axis in
//! `i128`, rounded half away from zero; a page point maps to the sheet by `offset + page × scale_ppb / 10⁹`,
//! rounded to the nearest nanometre (half away from zero).
//!
//! **`{page_index}`** is the xref engine's: the 1-based position in the page order counting every page record
//! but not report continuation pages (doc 06 §2.8, FR-XRF-003), so a printed cell and Go to agree with the
//! cross-reference texts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_rds::{Aspect, RdsSyntax};
use ew_derive::ew_schema::{FrameGrid, Function, Page, PageType, RepType, Representation};
use ew_derive::ew_units::{Grid, Nm, Pt2};
use ew_derive::{
    CpRef, FrameGridOf, FunctionCps, ItemOptions, ItemValues, PageEntry, PageOrder, PageOrderV, PageRef,
    PageStructure, PageTitle, Q, RepAnchorCell, XrefSettingsOf, XrefToken, cell_in_grid, cp_cell, cp_label,
    cp_point, render_item,
};
use ew_dl::{Destination, DisplayList, Item, ItemKind, Link, Transform};

use super::view::{PageFrameOf, PageGridOf, PageTableOf, neutral, page_uuid};
use super::{CpTargetsByPage, page_scene};
use crate::Session;

/// The connection grid of QElectroTech's 10 px, `7_937_500/3` nm (ADR-0027).
const QET_GRID: (u64, u64) = (7_937_500, 3);

/// Parts per 10⁹: the unit of a page-to-sheet scale.
const PPB_ONE: i128 = 1_000_000_000;

/// One row of the page navigator: an entry of the derived page order (WP-S2.03 §6.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageRow {
    /// The page's stable ID ([`super::page_uuid`]): a drawn page's ID, a divider's section ID, a generated page's
    /// v5 ID.
    pub id: Uuid,
    /// The printed folio label.
    pub folio: String,
    /// The neutral page title without the folio.
    pub title: String,
    /// The page type's snake_case spelling (`multi_line`, `report`).
    pub kind: String,
    /// The position of the page type in the schema's `PageType` declaration (sort by type).
    pub kind_rank: u16,
    /// Whether the page is a generated report page.
    pub generated: bool,
    /// The section's number and neutral title; `None` for front matter.
    pub section: Option<(u32, String)>,
    /// The page's `=` designation as printed (`=P01`); `None` when unassigned.
    pub function: Option<String>,
    /// The page's `+` designation as printed (`+J-E-2416`); `None` when unassigned.
    pub location: Option<String>,
    /// Whether the page's connection grid is the QET grid `7_937_500/3` nm.
    pub qet_grid: bool,
    /// The label of the revision that last changed the page (doc 15 §5.6, `last_changed_in`; WP-S1.18 part 2, the
    /// navigator's "Changed in" column); `None` without revisions.
    pub changed_in: Option<String>,
}

/// A cell of a page's frame grid: its printed labels and its 0-based position in the grid. Ordering uses the
/// indices, never the labels (which order `10` before `2`, and custom labels arbitrarily).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CellLabel {
    /// The printed row label (`H`).
    pub row: String,
    /// The printed column label (`15`).
    pub column: String,
    /// The 0-based row in the frame grid.
    pub row_index: u32,
    /// The 0-based column in the frame grid.
    pub column_index: u32,
}

impl fmt::Display for CellLabel {
    /// Row then column: `H15`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.row, self.column)
    }
}

/// The page-to-sheet map of a page's frame: `sheet = offset + page × scale_ppb / 10⁹`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Map {
    scale_ppb: i128,
    offset: (i128, i128),
}

/// `num / den` rounded to the nearest integer, halves away from zero (`den > 0`).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() >= den { q + num.signum() } else { q }
}

/// An `i128` as `i64`, saturating.
fn sat(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

impl Map {
    /// The page coordinate of the sheet coordinate `s` on axis `axis` (0 x, 1 y).
    fn to_page(self, s: i64, axis: usize) -> i64 {
        let off = if axis == 0 { self.offset.0 } else { self.offset.1 };
        sat(div_round((i128::from(s) - off) * PPB_ONE, self.scale_ppb.max(1)))
    }

    /// The sheet coordinate of the exact page coordinate `num / den` nm on `axis`, to the nearest nm.
    fn to_sheet(self, num: i128, den: i128, axis: usize) -> i64 {
        let off = if axis == 0 { self.offset.0 } else { self.offset.1 };
        sat(off + div_round(num * self.scale_ppb, den * PPB_ONE))
    }
}

/// A page's frame cells for a viewer (WP-S2.03 §4.2): opaque, made by [`Session::page_cells`]; evaluating it never
/// touches the `Session`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageCells {
    folio: String,
    page_index: Option<usize>,
    frame: FrameGrid,
    grid: Grid,
    map: Map,
    ip: Vec<XrefToken>,
    rows: Vec<String>,
    columns: Vec<String>,
}

/// The printed column labels of `frame` in grid order.
fn column_labels(frame: &FrameGrid) -> Vec<String> {
    if frame.columns.labels.is_empty() {
        (0..u32::from(frame.columns.count))
            .map(|c| (u32::from(frame.columns.start_index) + c).to_string())
            .collect()
    } else {
        frame
            .columns
            .labels
            .iter()
            .take(usize::from(frame.columns.count))
            .map(|l| l.as_str().to_owned())
            .collect()
    }
}

/// The printed row labels of `frame` in grid order (letters `A … Z`, skipping `I` and `O` when asked; rows beyond
/// the letters have no label, as in `ew_derive`'s cell rule).
fn row_labels(frame: &FrameGrid) -> Vec<String> {
    if frame.rows.labels.is_empty() {
        ('A'..='Z')
            .filter(|c| !(frame.rows.skip_io && matches!(c, 'I' | 'O')))
            .take(usize::from(frame.rows.count))
            .map(String::from)
            .collect()
    } else {
        frame
            .rows
            .labels
            .iter()
            .take(usize::from(frame.rows.count))
            .map(|l| l.as_str().to_owned())
            .collect()
    }
}

impl PageCells {
    /// The cell under the sheet point `sheet` (nm), by `ew_derive`'s exact rule on the page point; `None` outside
    /// the drawing area.
    pub fn cell_at(&self, sheet: (i64, i64)) -> Option<CellLabel> {
        let page = Pt2::new(Nm(self.map.to_page(sheet.0, 0)), Nm(self.map.to_page(sheet.1, 1)));
        let cell = cell_in_grid(&self.frame, self.grid, page)?;
        Some(CellLabel {
            row: cell.row_label,
            column: cell.column_label,
            row_index: cell.row,
            column_index: cell.column - u32::from(self.frame.columns.start_index),
        })
    }

    /// The position of a cell given by its labels (ASCII case-insensitively), `(row, column)`.
    fn position(&self, cell: &CellLabel) -> Option<(usize, usize)> {
        let row = self.rows.iter().position(|r| r.eq_ignore_ascii_case(&cell.row))?;
        let column = self
            .columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&cell.column))?;
        Some((row, column))
    }

    /// The cell at grid position `(row, column)`, with its printed labels and indices (both in range).
    fn label(&self, row: usize, column: usize) -> CellLabel {
        CellLabel {
            row: self.rows[row].clone(),
            column: self.columns[column].clone(),
            row_index: u32::try_from(row).unwrap_or(u32::MAX),
            column_index: u32::try_from(column).unwrap_or(u32::MAX),
        }
    }

    /// The header band as the exact rational `a/b` nm.
    fn origin(&self) -> (i128, i128) {
        self.frame
            .header_band
            .map_or((0, 1), |h| (i128::from(h.num()), i128::from(h.den())))
    }

    /// The cell's rectangle on the sheet, `[x0, y0, x1, y1]` nm, each edge the exact page edge mapped to the sheet
    /// and rounded to the nearest nm; `None` when the page has no cell with these labels (matched ASCII
    /// case-insensitively).
    pub fn cell_rect(&self, cell: &CellLabel) -> Option<[i64; 4]> {
        let (row, column) = self.position(cell)?;
        let (a, b) = self.origin();
        let edge = |k: usize, step: Grid, axis: usize| {
            let (c, d) = (i128::from(step.num()), i128::from(step.den()));
            let k = k as i128;
            // a/b + k·c/d = (a·d + k·c·b) / (b·d)
            self.map.to_sheet(a * d + k * c * b, b * d, axis)
        };
        let (w, h) = (self.frame.columns.width, self.frame.rows.height);
        Some([
            edge(column, w, 0),
            edge(row, h, 1),
            edge(column + 1, w, 0),
            edge(row + 1, h, 1),
        ])
    }

    /// The cell as the project's `ip` cross-reference format prints it for this page (`102-H15`), with
    /// `{function}` and `{location}` empty and `{page_index}` the page's position.
    pub fn text(&self, cell: &CellLabel) -> String {
        let values = ItemValues {
            page: self.folio.clone(),
            page_index: self.page_index,
            row: Some(cell.row.clone()),
            column: Some(cell.column.clone()),
            function: String::new(),
            location: String::new(),
        };
        render_item(&self.ip, &values, &ItemOptions::default())
    }

    /// The column width and the row height on the sheet, rounded to nm.
    pub fn pitch_nm(&self) -> (i64, i64) {
        let len = |g: Grid| {
            sat(div_round(
                i128::from(g.num()) * self.map.scale_ppb,
                i128::from(g.den()) * PPB_ONE,
            ))
        };
        (len(self.frame.columns.width), len(self.frame.rows.height))
    }
}

/// Where Go to lands (WP-S2.03 §6.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GotoTarget {
    /// The page's stable ID ([`PageRow::id`]).
    pub page: Uuid,
    /// Its printed folio label.
    pub folio: String,
    /// The cell, when the text named one.
    pub cell: Option<CellLabel>,
    /// The cell's rectangle on the sheet ([`PageCells::cell_rect`]).
    pub rect_nm: Option<[i64; 4]>,
}

/// Why Go to found nothing.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GotoError {
    /// The text is empty.
    Empty,
    /// No page, or more than one reading of the text.
    NotFound,
    /// The text names a page, but no cell of it.
    NoSuchCell {
        /// The page's folio label.
        folio: String,
        /// The cell part of the text.
        cell: String,
    },
}

/// The target of a cross-reference link (WP-S2.03 §4.2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LinkTarget {
    /// The target page's stable ID.
    pub page: Uuid,
    /// Its printed folio label.
    pub folio: String,
    /// The target item's anchor on the sheet, nm (the PDF's named destination); `None` for a page destination.
    pub at_nm: Option<(i64, i64)>,
    /// The target's cell as its cross-reference prints it (`103-A1`); `None` for a page destination or a target
    /// without a cell.
    pub cell: Option<String>,
}

/// A connection point drawn on a page (WP-S2.03 §4.2, part 2): what the canvas tools snap to and the keyboard
/// steps through.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageCp {
    /// The CP's v5 ID, `Destination::Cp`'s.
    pub id: Uuid,
    /// The CP as `cp_label` prints it (`-102F1:2`).
    pub label: String,
    /// Its point on the sheet, nm (`cp_point` under the page-to-sheet map, rounded to the nearest nm).
    pub at_nm: (i64, i64),
    /// The index `k` of the page's connection grid when the page point lies within 1 nm of `k × g` on both axes.
    pub grid_index: Option<(i64, i64)>,
    /// Whether a connection ends at it.
    pub connected: bool,
}

/// One page as Go to reads it: its folio, its position and its frame's labels.
struct Candidate<'a> {
    folio: &'a str,
    page_index: Option<usize>,
    rows: &'a [String],
    columns: &'a [String],
}

/// One reading of a Go to text: the page (by its position among the candidates) and the cell.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Reading {
    /// A page and its cell, `(row, column)`.
    Cell(usize, usize, usize),
    /// A page whose cell part, the text there, names no label of it.
    Unknown(usize, String),
}

/// What the cell tokens accept: the candidate's labels, or any non-empty text (the second pass).
#[derive(Clone, Copy, PartialEq, Eq)]
enum CellMode {
    Labels,
    Any,
}

/// Every reading of `text` against `tokens` for candidate `n` (see [`Session::resolve_goto`]).
fn readings(
    tokens: &[XrefToken],
    text: &str,
    n: usize,
    c: &Candidate<'_>,
    mode: CellMode,
) -> BTreeSet<Reading> {
    let mut out = BTreeSet::new();
    walk(
        tokens,
        text,
        c,
        mode,
        (None, None),
        String::new(),
        &mut |row, col, cell| match (mode, row, col) {
            (CellMode::Labels, Some(r), Some(k)) => {
                out.insert(Reading::Cell(n, r, k));
            }
            (CellMode::Any, _, _) => {
                out.insert(Reading::Unknown(n, cell.to_owned()));
            }
            _ => {}
        },
    );
    out
}

/// The receiver of a complete reading: the row and column found (by label position) and the cell text.
type Found<'a> = dyn FnMut(Option<usize>, Option<usize>, &str) + 'a;

/// The backtracking matcher of [`readings`]: `cell` collects the text the row and column tokens consumed.
fn walk(
    tokens: &[XrefToken],
    text: &str,
    c: &Candidate<'_>,
    mode: CellMode,
    at: (Option<usize>, Option<usize>),
    cell: String,
    found: &mut Found<'_>,
) {
    let Some((token, rest)) = tokens.split_first() else {
        if text.is_empty() {
            found(at.0, at.1, &cell);
        }
        return;
    };
    // The non-empty prefixes of `text` at character boundaries.
    let prefixes = || {
        text.char_indices()
            .skip(1)
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .filter(|&i| i > 0)
    };
    match token {
        XrefToken::Lit(s) => {
            if let Some(after) = text.strip_prefix(s.as_str()) {
                walk(rest, after, c, mode, at, cell, found);
            }
        }
        XrefToken::Page => {
            if let Some(after) = text.strip_prefix(c.folio) {
                walk(rest, after, c, mode, at, cell, found);
            }
        }
        XrefToken::PageIndex => {
            let digits = text.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 && text[..digits].parse::<usize>().ok() == c.page_index {
                walk(rest, &text[digits..], c, mode, at, cell, found);
            }
        }
        XrefToken::Function | XrefToken::Location => {
            for i in prefixes() {
                walk(rest, &text[i..], c, mode, at, cell.clone(), found);
            }
        }
        XrefToken::Row | XrefToken::Column => {
            let labels = if *token == XrefToken::Row {
                c.rows
            } else {
                c.columns
            };
            for i in prefixes() {
                let part = &text[..i];
                let hit = labels.iter().position(|l| l.eq_ignore_ascii_case(part));
                if mode == CellMode::Labels && hit.is_none() {
                    continue;
                }
                let mut next = at;
                if *token == XrefToken::Row {
                    next.0 = hit;
                } else {
                    next.1 = hit;
                }
                walk(rest, &text[i..], c, mode, next, format!("{cell}{part}"), found);
            }
        }
        // `{list}` is a wrapper's token: it reads nothing.
        XrefToken::List => walk(rest, text, c, mode, at, cell, found),
    }
}

/// The reading of a Go to text over `candidates`, page order first (WP-S2.03 §4.2): `Ok((candidate, row,
/// column))` for a unique full reading; `Err(Some((candidate, cell)))` when no reading names a cell and exactly
/// one page reads with an unknown cell part; `Err(None)` otherwise.
fn read_goto(
    tokens: &[XrefToken],
    text: &str,
    candidates: &[Candidate<'_>],
) -> Result<(usize, usize, usize), Option<(usize, String)>> {
    let mut full = BTreeSet::new();
    for (n, c) in candidates.iter().enumerate() {
        full.extend(readings(tokens, text, n, c, CellMode::Labels));
        if full.len() > 1 {
            return Err(None);
        }
    }
    if let Some(Reading::Cell(n, r, k)) = full.into_iter().next() {
        return Ok((n, r, k));
    }
    let mut partial = BTreeMap::new();
    for (n, c) in candidates.iter().enumerate() {
        for r in readings(tokens, text, n, c, CellMode::Any) {
            if let Reading::Unknown(n, cell) = r {
                partial.entry(n).or_insert(cell);
            }
        }
        if partial.len() > 1 {
            return Err(None);
        }
    }
    Err(partial.into_iter().next())
}

/// The `{page_index}` of the entry at `pos` for Go to and printed cells: the xref engine's count, and `None` for a
/// report continuation page ("2.1"), which has no page record and so no index of its own.
fn record_index(order: &PageOrderV, pos: usize) -> Option<usize> {
    match order.entries[pos].page {
        PageRef::Generated { index, .. } if index > 0 => None,
        _ => Some(page_index_of(order, pos)),
    }
}

/// The xref engine's `{page_index}` of the entry at `pos`: the 1-based count of page records up to it, report
/// continuation pages left out (doc 06 §2.8).
fn page_index_of(order: &PageOrderV, pos: usize) -> usize {
    order.entries[..pos]
        .iter()
        .filter(|e| !matches!(e.page, PageRef::Generated { index, .. } if index > 0))
        .count()
        + 1
}

/// The position of a page type in the schema's declaration of `PageType`.
fn kind_rank(kind: &PageType) -> u16 {
    match kind {
        PageType::TitlePage => 0,
        PageType::TableOfContents => 1,
        PageType::SectionDivider => 2,
        PageType::Graphic => 3,
        PageType::SingleLine => 4,
        PageType::MultiLine => 5,
        PageType::Overview => 6,
        PageType::BusTopology => 7,
        PageType::Fluid => 8,
        PageType::AssemblyOverview => 9,
        PageType::PanelLayout => 10,
        PageType::ModelView => 11,
        PageType::ExternalDocument => 12,
        PageType::Report => 13,
        PageType::Unknown(_) => 14,
    }
}

/// The page type's snake_case spelling (a string of a newer format as it is, `unknown` for any other value).
fn kind_name(kind: &PageType) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// One aspect of a drawn page's structure as printed, `None` when unassigned.
fn aspect(q: &Q, page: PageRef, aspect: Aspect) -> Option<String> {
    let PageRef::Drawn(p) = page else {
        return None;
    };
    let text = q
        .get::<PageStructure>(p)
        .render_aspect(aspect, &RdsSyntax::default());
    (!text.is_empty()).then_some(text)
}

/// The page-to-sheet map of a page's frame form (the identity without one).
fn sheet_map(q: &Q, page: PageRef) -> Map {
    let form = q.get::<PageFrameOf>(page);
    form.as_ref()
        .as_ref()
        .and_then(|f| f.form.page_to_sheet.as_ref())
        .map_or(
            Map {
                scale_ppb: PPB_ONE,
                offset: (0, 0),
            },
            |t| Map {
                scale_ppb: i128::from(t.scale_ppb),
                offset: (i128::from(t.offset.x.0), i128::from(t.offset.y.0)),
            },
        )
}

/// The index of the grid point within 1 nm of the page coordinate `p` on a grid of `num / den` nm, if any.
fn grid_index(p: i64, num: i128, den: i128) -> Option<i64> {
    if num <= 0 || den <= 0 {
        return None;
    }
    let k = div_round(i128::from(p) * den, num);
    ((k * num - i128::from(p) * den).abs() <= den).then(|| sat(k))
}

/// The reading-order key of a CP: its cell's column and row index and its sheet `(y, x)`. `page_cps` breaks ties by
/// the label (natural order, then bytes) and only then by the ID, so two coincident CPs keep their order in every
/// fresh copy of a project (WP-S6.13 §6 item 3: the IDs of a new project are random).
type ReadingKey = (u32, u32, i64, i64);

/// The function CPs a page may draw: every CP of every function its representations name (their functions and
/// pin-map targets) and, for a strip segment, of every terminal of its strip. [`cp_point`] decides which are
/// drawn.
fn cp_candidates(q: &Q, page: Id<Page>) -> BTreeSet<(Id<Function>, u16)> {
    let mut functions: BTreeSet<Id<Function>> = BTreeSet::new();
    for rid in q.idx().reps_by_page(page) {
        let Some(rep) = q.reps().get(rid) else { continue };
        functions.extend(rep.functions.iter().copied());
        functions.extend(rep.pin_map.as_slice().iter().map(|p| p.function));
        if rep.rep_type == RepType::StripSegment
            && let Some(strip) = rep
                .functions
                .first()
                .and_then(|f| q.functions().get(*f))
                .and_then(|f| f.device)
        {
            for terminal in q.idx().terminals_of_strip(strip) {
                functions.extend(q.idx().functions_of_device(terminal));
            }
        }
    }
    let mut out = BTreeSet::new();
    for f in functions {
        for cp in q.get::<FunctionCps>(f).iter() {
            out.insert((f, cp.idx));
        }
    }
    out
}

/// The cells of the entry at `pos` of `order` (see [`Session::page_cells`]).
fn cells_of(q: &Q, order: &PageOrderV, pos: usize) -> Option<PageCells> {
    let entry = &order.entries[pos];
    let PageRef::Drawn(p) = entry.page else {
        return None;
    };
    let frame = q.get::<FrameGridOf>(p).as_ref().clone()?;
    let grid = q.pages().get(p)?.grid;
    let map = sheet_map(q, entry.page);
    let ip = q.get::<XrefSettingsOf>(()).ip.clone();
    Some(PageCells {
        folio: entry.folio.clone(),
        page_index: record_index(order, pos),
        rows: row_labels(&frame),
        columns: column_labels(&frame),
        frame,
        grid,
        map,
        ip,
    })
}

/// The anchor of a link target as the PDF writer takes it (`ew-export`'s `pdf.rs`): an instance's insertion
/// point, a run's origin in its parent, a path's first point, an image's corner, a group's translation.
fn anchor(item: &Item) -> Option<Pt2> {
    match &item.kind {
        ItemKind::Instance(i) => Some(i.transform.translate),
        ItemKind::Glyphs(run) => Some(run.to_parent(Pt2::ORIGIN)),
        ItemKind::Path(p) => p.path.points().next(),
        ItemKind::Image(img) => Some(img.rect.min()),
        ItemKind::Group(g) => Some(g.transform.unwrap_or(Transform::IDENTITY).translate),
    }
}

/// The sheet anchor of the first `Link::Target(dest)` of `items` in the PDF writer's scan order (pre-order;
/// instances are not resolved: the sheet list has them inlined).
fn find_target(items: &[Item], dest: Destination, stack: &[Transform]) -> Option<Pt2> {
    for item in items {
        if item.link == Some(Link::Target(dest))
            && let Some(a) = anchor(item)
        {
            return Some(stack.iter().rev().fold(a, |q, t| t.apply(q)));
        }
        if let ItemKind::Group(g) = &item.kind {
            let mut s = stack.to_vec();
            s.push(g.transform.unwrap_or(Transform::IDENTITY));
            if let Some(p) = find_target(&g.items, dest, &s) {
                return Some(p);
            }
        }
    }
    None
}

/// The xref engine's rendering of a target cell with the `ip` format: the target page's folio, its
/// `{page_index}`, its structure for `{function}` and `{location}`, no source-relative prefix.
fn printed_cell(
    q: &Q,
    order: &PageOrderV,
    pos: usize,
    tokens: &[XrefToken],
    cell: Option<&ew_derive::Cell>,
) -> Option<String> {
    let cell = cell?;
    let entry = &order.entries[pos];
    let values = ItemValues {
        page: entry.folio.clone(),
        page_index: tokens
            .contains(&XrefToken::PageIndex)
            .then(|| page_index_of(order, pos)),
        row: Some(cell.row_label.clone()),
        column: Some(cell.column_label.clone()),
        function: aspect(q, entry.page, Aspect::Function).unwrap_or_default(),
        location: aspect(q, entry.page, Aspect::Location).unwrap_or_default(),
    };
    Some(render_item(tokens, &values, &ItemOptions::default()))
}

/// The resolution of link targets within one [`Session::link_targets`] call: the sheet display list of each
/// target page, built once.
struct Targets<'q> {
    q: &'q Q,
    order: Arc<PageOrderV>,
    lists: BTreeMap<usize, Arc<DisplayList>>,
}

impl Targets<'_> {
    /// The sheet display list of the entry at `pos` (the memoised layers, as the canvas's page content builds it).
    fn list(&mut self, pos: usize) -> Arc<DisplayList> {
        let (q, order) = (self.q, &self.order);
        Arc::clone(
            self.lists
                .entry(pos)
                .or_insert_with(|| Arc::new(page_scene(q, &order.entries[pos]).sheet_list())),
        )
    }

    /// The first page among `pages` (positions in page order, ascending) whose sheet list holds the target of
    /// `dest`, with the target's sheet anchor.
    fn locate(&mut self, dest: Destination, pages: &[usize]) -> Option<(usize, (i64, i64))> {
        pages.iter().find_map(|&pos| {
            let list = self.list(pos);
            find_target(&list.items, dest, &[]).map(|p| (pos, (p.x.0, p.y.0)))
        })
    }

    fn row(&self, pos: usize, at_nm: Option<(i64, i64)>, cell: Option<String>) -> LinkTarget {
        let e: &PageEntry = &self.order.entries[pos];
        LinkTarget {
            page: page_uuid(e.page),
            folio: e.folio.clone(),
            at_nm,
            cell,
        }
    }
}

/// The printed table of a generated page ([`Session::page_list_table`], FT-660): what a text-only reader needs of a
/// list page.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageListTable {
    /// The column headers, in order.
    pub columns: Vec<String>,
    /// The rows of the page, in order.
    pub rows: Vec<PageListRow>,
    /// The text printed instead of rows (a row source not derived yet, an empty block).
    pub note: Option<String>,
}

/// One printed row of a [`PageListTable`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PageListRow {
    /// A heading (a group header or footer): its first cell spans every column.
    pub heading: bool,
    /// One text per column (a heading uses the first).
    pub cells: Vec<String>,
}

impl Session {
    /// The rows of the page navigator: one per entry of the derived page order, in that order (the entries of
    /// [`Session::page_labels`], same count and folios; WP-S2.03 §4.2).
    pub fn page_rows(&self) -> Vec<PageRow> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        order
            .entries
            .iter()
            .map(|e| {
                let grid = *q.get::<PageGridOf>(e.page);
                PageRow {
                    id: page_uuid(e.page),
                    folio: e.folio.clone(),
                    title: neutral(&q.get::<PageTitle>(e.page)),
                    kind: kind_name(&e.kind),
                    kind_rank: kind_rank(&e.kind),
                    generated: matches!(e.page, PageRef::Generated { .. }),
                    section: e
                        .section
                        .and_then(|s| q.sections().get(s))
                        .map(|s| (u32::from(s.number), neutral(&s.title))),
                    function: aspect(&q, e.page, Aspect::Function),
                    location: aspect(&q, e.page, Aspect::Location),
                    qet_grid: (grid.num(), grid.den()) == QET_GRID,
                    changed_in: super::revisions::last_changed_in(&q, e.page),
                }
            })
            .collect()
    }

    /// The printed table of the generated page labelled `folio` (a parts list, a cable list, a table of contents):
    /// the column headers and the page's rows as the folio prints them (`page_table`, FT-660); `None` when no page has
    /// that folio or the page is drawn or a divider.
    pub fn page_list_table(&self, folio: &str) -> Option<PageListTable> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let entry = order.entries.iter().find(|e| e.folio == folio)?;
        let table = q.get::<PageTableOf>(entry.page);
        let t = table.as_ref().as_ref()?;
        Some(PageListTable {
            columns: t.columns.iter().map(|c| c.header.clone()).collect(),
            rows: t
                .rows
                .iter()
                .map(|r| PageListRow {
                    heading: r.kind == ew_scene::table::TableRowKind::Heading,
                    cells: r.cells.clone(),
                })
                .collect(),
            note: t.note.clone(),
        })
    }

    /// The frame cells of the page labelled `folio` ([`PageCells`]); `None` when no page has that folio or the page
    /// has no frame grid.
    pub fn page_cells(&self, folio: &str) -> Option<PageCells> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let pos = order.entries.iter().position(|e| e.folio == folio)?;
        cells_of(&q, &order, pos)
    }

    /// Go to page or cell (FR-UI-039; WP-S2.03 §4.2): the trimmed `text` is a printed folio label, or a cell in the
    /// project's `ip` cross-reference format (`103-A1`; `/103.A1` with the EPLAN cell preset).
    ///
    /// The parse tries the folios in page order: `{page}` matches a folio label exactly, `{page_index}` decimal
    /// digits naming the page's 1-based position, `{row}` and `{column}` labels of that page's frame (ASCII
    /// case-insensitively), `{function}` and `{location}` any non-empty text (ignored), literals exactly; the whole
    /// text must be consumed. A text is accepted only when exactly one (folio, row, column) reading exists.
    ///
    /// # Errors
    ///
    /// [`GotoError::Empty`] for an empty text; [`GotoError::NoSuchCell`] when the only reading names a page and a
    /// cell part that is no cell of it; [`GotoError::NotFound`] for anything else, two or more readings included.
    pub fn resolve_goto(&self, text: &str) -> Result<GotoTarget, GotoError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(GotoError::Empty);
        }
        let q = self.query();
        let order = q.get::<PageOrder>(());
        if let Some(e) = order.entries.iter().find(|e| e.folio == text) {
            return Ok(GotoTarget {
                page: page_uuid(e.page),
                folio: e.folio.clone(),
                cell: None,
                rect_nm: None,
            });
        }
        let tokens = q.get::<XrefSettingsOf>(()).ip.clone();
        let cells: Vec<Option<PageCells>> = (0..order.entries.len())
            .map(|pos| cells_of(&q, &order, pos))
            .collect();
        let none: Vec<String> = Vec::new();
        let candidates: Vec<Candidate<'_>> = order
            .entries
            .iter()
            .enumerate()
            .map(|(pos, e)| Candidate {
                folio: &e.folio,
                page_index: record_index(&order, pos),
                rows: cells[pos].as_ref().map_or(&none, |c| &c.rows),
                columns: cells[pos].as_ref().map_or(&none, |c| &c.columns),
            })
            .collect();
        match read_goto(&tokens, text, &candidates) {
            Ok((pos, row, column)) => {
                let Some(c) = &cells[pos] else {
                    return Err(GotoError::NotFound);
                };
                let cell = c.label(row, column);
                let e = &order.entries[pos];
                Ok(GotoTarget {
                    page: page_uuid(e.page),
                    folio: e.folio.clone(),
                    rect_nm: c.cell_rect(&cell),
                    cell: Some(cell),
                })
            }
            Err(Some((pos, cell))) => Err(GotoError::NoSuchCell {
                folio: order.entries[pos].folio.clone(),
                cell,
            }),
            Err(None) => Err(GotoError::NotFound),
        }
    }

    /// The targets of cross-reference link destinations (WP-S2.03 §4.2), one result per input, in order.
    ///
    /// A representation or CP destination resolves to the page whose sheet display list holds its
    /// `Link::Target` item (the first in page order, as the PDF writer resolves it), `at_nm` that item's anchor on
    /// the sheet, and `cell` the target's cell (`rep_anchor_cell`, `cp_cell`) printed as its cross-reference prints
    /// it (the project's `ip` format, the target page's `{function}` and `{location}`). A page destination is that
    /// page, without a point or cell. `None` for a destination outside this project's page order or one that does
    /// not exist.
    ///
    /// The page lookup and the cell read memoised families; at most one sheet display list is built per distinct
    /// target page, never one of another page.
    pub fn link_targets(&self, dests: &[Destination]) -> Vec<Option<LinkTarget>> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let tokens = q.get::<XrefSettingsOf>(()).ip.clone();
        let mut t = Targets {
            q: &q,
            order: Arc::clone(&order),
            lists: BTreeMap::new(),
        };
        // The CP destinations of the project: v5(function, idx) → the drawn pages that hold a target of it, with
        // the function and index, read once and only when a CP is asked for.
        let mut cps: Option<CpDestinations> = None;
        dests
            .iter()
            .map(|&d| match d {
                Destination::Page(id) => {
                    let pos = order.entries.iter().position(|e| page_uuid(e.page) == id)?;
                    Some(t.row(pos, None, None))
                }
                Destination::Representation(id) => {
                    let rid: Id<Representation> = Id::from_uuid(id);
                    let page = q.reps().get(rid)?.page;
                    let pos = order.position(PageRef::Drawn(page))?;
                    let (pos, at) = t.locate(d, &[pos])?;
                    let cell = q.get::<RepAnchorCell>(rid);
                    let cell = printed_cell(&q, &order, pos, &tokens, cell.as_ref().as_ref());
                    Some(t.row(pos, Some(at), cell))
                }
                Destination::Cp(id) => {
                    let map = cps.get_or_insert_with(|| cp_destinations(&q, &order));
                    let (function, idx, pages) = map.get(&id)?.clone();
                    let (pos, at) = t.locate(d, &pages)?;
                    let PageRef::Drawn(page) = order.entries[pos].page else {
                        return None;
                    };
                    let cell = cp_cell(&q, function, idx, page);
                    let cell = printed_cell(&q, &order, pos, &tokens, cell.as_ref());
                    Some(t.row(pos, Some(at), cell))
                }
            })
            .collect()
    }

    /// Every connection point drawn on the page labelled `folio` (WP-S2.03 §4.2, part 2), in reading order: by the
    /// column index, then the row index of the CP's cell (`cp_cell`; a CP outside the frame grid last), then by the
    /// sheet point's `(y, x)`, then by the label (natural order, then bytes), then by ID (coincident CPs keep their
    /// order in every fresh copy of a project). Empty for an unknown folio, a page that is not drawn, or a page
    /// without CPs.
    ///
    /// `at_nm` is `cp_point` mapped to the sheet by the frame's page-to-sheet map and rounded to the nearest nm;
    /// `grid_index` the grid index `k` per axis when the page point lies within 1 nm of `k × g` (always, by
    /// FR-SYM-009, for a symbol CP); `connected` whether a connection ends at the CP. Reads memoised families only;
    /// a viewer fetches it once per (revision, folio) with the page content.
    pub fn page_cps(&self, folio: &str) -> Vec<PageCp> {
        let q = self.query();
        let order = q.get::<PageOrder>(());
        let Some(entry) = order.entries.iter().find(|e| e.folio == folio) else {
            return Vec::new();
        };
        let PageRef::Drawn(page) = entry.page else {
            return Vec::new();
        };
        let Some(grid) = q.pages().get(page).map(|p| p.grid) else {
            return Vec::new();
        };
        let (num, den) = (i128::from(grid.num()), i128::from(grid.den()));
        let map = sheet_map(&q, entry.page);
        let start = q
            .get::<FrameGridOf>(page)
            .as_ref()
            .as_ref()
            .map_or(0, |f| u32::from(f.columns.start_index));
        let mut rows: Vec<(ReadingKey, PageCp)> = Vec::new();
        for (function, idx) in cp_candidates(&q, page) {
            let Some(p) = cp_point(&q, function, idx, page) else {
                continue;
            };
            let at_nm = (
                map.to_sheet(i128::from(p.x.0), 1, 0),
                map.to_sheet(i128::from(p.y.0), 1, 1),
            );
            let grid_index = grid_index(p.x.0, num, den).zip(grid_index(p.y.0, num, den));
            let id: Id<()> = ew_derive::ew_ids::v5::cp_id(function, u32::from(idx));
            let (column, row) = cp_cell(&q, function, idx, page)
                .map_or((u32::MAX, u32::MAX), |c| (c.column.saturating_sub(start), c.row));
            let cp = PageCp {
                id: id.as_uuid(),
                label: cp_label(&q, CpRef { function, idx }),
                at_nm,
                grid_index,
                connected: q.idx().connections_at_cp(function, idx).next().is_some(),
            };
            rows.push(((column, row, at_nm.1, at_nm.0), cp));
        }
        rows.sort_by(|(ka, a), (kb, b)| {
            ka.cmp(kb)
                .then_with(|| ew_mltext::natural_cmp(&a.label, &b.label))
                .then_with(|| a.label.cmp(&b.label))
                .then_with(|| a.id.cmp(&b.id))
        });
        rows.into_iter().map(|(_, cp)| cp).collect()
    }
}

/// The CP destinations of a project: v5 ID → (function, CP index, the page-order positions of the pages that
/// hold a target of it).
type CpDestinations = BTreeMap<Uuid, (Id<Function>, u16, Vec<usize>)>;

/// Every CP destination of the project's pages (`cp_targets_by_page`): its v5 ID, the function and index, and the
/// positions in page order of the drawn pages that hold a target of it, ascending.
fn cp_destinations(q: &Q, order: &PageOrderV) -> CpDestinations {
    let mut out = CpDestinations::new();
    for (page, targets) in q.get::<CpTargetsByPage>(()).iter() {
        let Some(pos) = order.position(PageRef::Drawn(*page)) else {
            continue;
        };
        for t in targets {
            let id: Id<()> = ew_derive::ew_ids::v5::cp_id(t.function, u32::from(t.idx));
            let entry = out
                .entry(id.as_uuid())
                .or_insert_with(|| (t.function, t.idx, Vec::new()));
            entry.2.push(pos);
        }
    }
    for (_, _, pages) in out.values_mut() {
        pages.sort_unstable();
        pages.dedup();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// `kind_rank` is the position of the page type in the schema's declaration of `PageType` (the navigator's sort
    /// by type): pinned against the committed schema, whose `oneOf` lists the variants in declaration order, with a
    /// type of a newer format after the last.
    #[test]
    fn kind_rank_is_the_schema_declaration_order_of_the_page_type() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../../schemas/0.1/page.schema.json")).unwrap();
        let names: Vec<&str> = schema["$defs"]["PageType"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["const"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 14, "{names:?}");
        for (i, name) in names.iter().enumerate() {
            let kind: PageType =
                serde_json::from_value(serde_json::Value::String((*name).to_owned())).unwrap();
            assert_eq!(usize::from(kind_rank(&kind)), i, "{name}");
            assert_eq!(kind_name(&kind), *name);
        }
        let newer = PageType::Unknown(serde_json::Value::String("future".to_owned()));
        assert_eq!(usize::from(kind_rank(&newer)), names.len());
        // A string of a newer format prints as it is; a value that is no string prints `unknown`.
        assert_eq!(kind_name(&newer), "future");
        assert_eq!(kind_name(&PageType::Unknown(serde_json::json!(7))), "unknown");
    }

    #[test]
    fn rounding_halves_away_from_zero() {
        assert_eq!(div_round(5, 2), 3);
        assert_eq!(div_round(-5, 2), -3);
        assert_eq!(div_round(4, 3), 1);
        assert_eq!(div_round(-4, 3), -1);
        assert_eq!(div_round(0, 7), 0);
        let m = Map {
            scale_ppb: 986_282_578,
            offset: (9_790_000, 9_790_000),
        };
        // The QET frame's header band, 15 875 000/3 nm, prints at 15.009 mm.
        assert_eq!(m.to_sheet(15_875_000, 3, 0), 15_009_079);
        assert_eq!(m.to_page(15_009_079, 0), 5_291_667);
    }

    #[test]
    fn the_parse_is_unique_or_not_found() {
        let qet = ew_derive::parse_format("{page}-{row}{column}");
        let (rows, cols) = (labels(&["A", "B"]), labels(&["0", "1", "10"]));
        let pages = [
            Candidate {
                folio: "101",
                page_index: Some(4),
                rows: &rows,
                columns: &cols,
            },
            Candidate {
                folio: "1-A",
                page_index: Some(1),
                rows: &rows,
                columns: &cols,
            },
        ];
        assert_eq!(read_goto(&qet, "101-a10", &pages), Ok((0, 0, 2)));
        assert_eq!(read_goto(&qet, "101-C1", &pages), Err(Some((0, "C1".to_owned()))));
        assert_eq!(read_goto(&qet, "102-A1", &pages), Err(None));
        // "1-A-A1" reads only as folio "1-A", row A, column 1.
        assert_eq!(read_goto(&qet, "1-A-A1", &pages), Ok((1, 0, 1)));
        // `{page}{row}{column}` with folios whose labels make two readings: "1011" is folio "1", row "0", column
        // "11", and folio "10", row "1", column "1". (Golden A's frames have letter rows and digit columns, which
        // always split one way; these synthetic frames do not.)
        let tight = ew_derive::parse_format("{page}{row}{column}");
        let (r1, c1) = (labels(&["0"]), labels(&["11"]));
        let (r2, c2) = (labels(&["1"]), labels(&["1"]));
        let two = [
            Candidate {
                folio: "1",
                page_index: Some(1),
                rows: &r1,
                columns: &c1,
            },
            Candidate {
                folio: "10",
                page_index: Some(2),
                rows: &r2,
                columns: &c2,
            },
        ];
        assert_eq!(read_goto(&tight, "1011", &two), Err(None), "ambiguous");
        assert_eq!(read_goto(&tight, "1011", &two[..1]), Ok((0, 0, 0)));
        // `{page_index}` is 1-based digits; `{function}` and `{location}` take any non-empty text.
        let indexed = ew_derive::parse_format("{function}/{page_index}.{row}{column}");
        assert_eq!(read_goto(&indexed, "=P01/4.B10", &pages), Ok((0, 1, 2)));
        assert_eq!(
            read_goto(&indexed, "/4.B10", &pages),
            Err(None),
            "function is not empty"
        );
    }

    #[test]
    fn the_labels_follow_the_frame() {
        let frame = ew_derive::default_frame_grid();
        assert_eq!(column_labels(&frame).len(), 17);
        assert_eq!(column_labels(&frame)[16], "16");
        assert_eq!(
            row_labels(&frame),
            labels(&["A", "B", "C", "D", "E", "F", "G", "H"])
        );
    }
}
