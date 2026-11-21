// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pagination of an arranged row set into generated pages (doc 09 §3.2, FR-REP-002, FR-PAG-011,
//! FR-PAG-030).
//!
//! ```text
//! paginate(rows, block, layouts):
//!   for group in rows.groups():                    # one implicit group when the block does not group
//!     NewPage break:  close the page if it has rows; the next page uses the next form of the chain
//!     NewFolio break: close the page; the next folio (previous + 1) restarts the form chain
//!     for row in group:
//!       h = height(row.kind) × lines(row)          # lines = 1 unless the kind's template wraps
//!       keep-with-next: a group header or heading needs room for itself and up to the next data row
//!       if used + need > capacity and the page has rows:
//!         close the page; open the next (continuation form)
//!         if the form has a group-header template and the group began with an emitted header row:
//!           reserve the "(cont.)" header and mark the page `continued_group`
//!       place(row)
//!   a run's last page under LAST_PAGE_ROWS data rows: the page before ends as many rows earlier, where it keeps as many
//!   no data rows: OmitPages → no pages, OnePage(text) → one page with that note
//!   source not derived yet → one placeholder page with the reason as its note
//!   number: first_folio, then SubPage (907.1, 907.2 …) or NextFolio (908, 909 …)
//!   title(page) = block.title with {n}, {count}, {group}
//! ```
//!
//! The functions here are pure over their inputs (no `Q`), so the algorithm is tested without a model; the
//! `report_block_pages` query ([`crate::BlockPages`]) feeds them the block's rows and form chain.
//!
//! # Table of contents
//!
//! A TOC block is not sliced by rows: [`toc_slices`] gives it the page count the `page_order` fixpoint
//! settled on, with `r` rows per page, where `r` is the TOC page capacity of decision D3: the TOC form's data
//! area (its capacity over its data-row height) when the form has a valid one, else the profile's
//! `toc.rows_per_page` (`crate::queries::toc_rows_per_page`, which `ReportEngine` also reports to
//! `page_order`).

use std::ops::Range;

use ew_derive::ew_ids::{Id, v5};
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{
    ContinuationPolicy, EmptyPolicy, GroupBreak, ReportBlock, ReportKind, ReportSource, Section,
};
use ew_derive::ew_units::Nm;
use ew_derive::{GeneratedPage, map_ml};

use crate::layout::{Layouts, PageLayout};
use crate::rowset::{Row, RowKind, RowSet, RowSetStatus};

/// Continuation numbering (FR-PAG-030).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Continuation {
    /// 907, 907.1, 907.2 … (the default).
    SubPage,
    /// 907, 908, 909 …
    NextFolio,
}

/// What happens at a group change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Break {
    /// Rows continue.
    None,
    /// A new page.
    NewPage,
    /// A new folio.
    NewFolio,
}

/// The block parameters pagination needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BlockSpec {
    /// The block.
    pub block: Id<ReportBlock>,
    /// The section of its pages.
    pub section: Option<Id<Section>>,
    /// The first folio number.
    pub first_folio: u32,
    /// Continuation numbering.
    pub continuation: Continuation,
    /// The page-title format ("Wire List {n}").
    pub title: MlText,
    /// The text of the one page a block without data rows prints; `None` omits the pages.
    pub empty: Option<MlText>,
    /// The group break.
    pub group_break: Break,
    /// The sub-page of the first page: 0 numbers it `first_folio` itself; a legend list starts after its layout page
    /// (`first_folio.k`, doc 09 §3.2).
    pub first_sub: u32,
}

impl BlockSpec {
    /// The pagination parameters of a page-oriented block; `None` for an embedded block or one without a
    /// first folio (the model's rule EW-INV-014 rules the latter out), which have no pages. A value of a
    /// newer format falls back to the default (sub-pages, omit when empty, no break).
    pub fn of_block(id: Id<ReportBlock>, block: &ReportBlock) -> Option<BlockSpec> {
        if block.embedded.is_some() {
            return None;
        }
        // A legend list's folio is its layout page's ([`legend_first_sub`]), which the caller sets.
        let first_folio = block.first_folio.or(block.layout_page.map(|_| 0))?;
        Some(BlockSpec {
            block: id,
            section: block.section,
            first_folio,
            continuation: match block.continuation {
                ContinuationPolicy::NextFolio => Continuation::NextFolio,
                ContinuationPolicy::SubPage | ContinuationPolicy::Unknown(_) => Continuation::SubPage,
            },
            title: block.title.clone(),
            empty: match &block.empty {
                EmptyPolicy::OnePage(text) => Some(text.clone()),
                EmptyPolicy::OmitPages | EmptyPolicy::Unknown(_) => None,
            },
            group_break: match block.group.as_ref().map(|g| &g.break_) {
                Some(GroupBreak::NewPage) => Break::NewPage,
                Some(GroupBreak::NewFolio) => Break::NewFolio,
                Some(GroupBreak::None | GroupBreak::Unknown(_)) | None => Break::None,
            },
            first_sub: 0,
        })
    }
}

/// The first sub-page of a legend list's run of `count` pages after its layout page, printed `layout` (`N` or `N.j`)
/// in a section whose drawn pages print `held` (doc 09 §3.2): the least k from 1 (from j + 1) at which none of `N.k` …
/// `N.(k + count − 1)` is held, so that no drawn page splits the run.
pub fn legend_first_sub(number: u32, layout: &str, held: &[String], count: u32) -> u32 {
    let head = number.to_string();
    let after = layout
        .strip_prefix(&head)
        .and_then(|rest| rest.strip_prefix('.'))
        .and_then(|j| j.parse::<u32>().ok())
        .map_or(1, |j| j.saturating_add(1));
    let taken = |k: u32| held.iter().any(|h| *h == format!("{head}.{k}"));
    (after..=u32::MAX)
        .find(|&k| (0..count.max(1)).all(|i| !taken(k.saturating_add(i))))
        .unwrap_or(after)
}

/// Whether a block is a table of contents (its page count comes from the `page_order` fixpoint).
pub fn is_toc(block: &ReportBlock) -> bool {
    matches!(block.source, ReportSource::Kind(ReportKind::Toc))
}

/// How many lines a row takes when its kind's template wraps (FR-REP-049): text measurement through
/// `ew-text`, which makes pagination depend on the display language. Rows of kinds that do not wrap take
/// one line and are never measured.
pub trait RowLines {
    /// The lines of `row` on a page of `layout` (at least 1 is used).
    fn lines(&self, row: &Row, layout: &PageLayout) -> u32;
}

/// Fixed-height rows: every row takes one line (the default; doc 09 §3.2).
#[derive(Clone, Copy, Debug, Default)]
pub struct FixedLines;

impl RowLines for FixedLines {
    fn lines(&self, _row: &Row, _layout: &PageLayout) -> u32 {
        1
    }
}

/// One page before numbering.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Slice {
    /// The rows of the page.
    pub rows: Range<u32>,
    /// The index (in [`RowSet::groups`]) of the page's first group.
    pub group: Option<usize>,
    /// Whether the page continues a group begun earlier (the form repeats its header, "(cont.)").
    pub continued_group: bool,
    /// The folio run (0 = the first folio; a `NewFolio` break starts the next).
    pub folio: u32,
    /// The page within its folio run (0 = the folio's first page); also its position in the form chain.
    pub sub: u32,
    /// The note printed instead of rows (an empty block's `OnePage` text, a not-derived placeholder).
    pub note: Option<MlText>,
}

/// Paginates an arranged row set: [`slice()`], then [`number()`].
pub fn paginate(
    set: &RowSet,
    spec: &BlockSpec,
    layouts: &Layouts,
    lines: &dyn RowLines,
) -> Vec<GeneratedPage> {
    let slices = slice(set, spec, layouts, lines);
    number(&slices, set, spec, layouts)
}

/// The fewest data rows the last page of a run of pages holds where the page before it can give them (doc 09 §3.2).
pub const LAST_PAGE_ROWS: usize = 3;

/// The pages of a row set, before numbering (see the module documentation).
///
/// An invalid layout of the chain ([`PageLayout::is_valid`]) is replaced by the house geometry
/// ([`Layouts::validated`]), so the page count is bounded by the row count at any geometry.
///
/// **The last page of a run** (the list's last page, or the last before a group that starts a page or a folio of its
/// own) holds at least [`LAST_PAGE_ROWS`] data rows where the page before it in the run can give them and keep as many:
/// that page then ends that many data rows earlier, so a list never ends on a page of one or two rows. The page count
/// stays the same.
pub fn slice(set: &RowSet, spec: &BlockSpec, layouts: &Layouts, lines: &dyn RowLines) -> Vec<Slice> {
    let layouts = &layouts.validated();
    if let RowSetStatus::NotDerived(reason) = &set.status {
        return vec![note_slice(MlText::neutral(&capitalise(&reason.to_string())))];
    }
    if set.data_rows() == 0 {
        return match &spec.empty {
            Some(text) => vec![note_slice(text.clone())],
            None => Vec::new(),
        };
    }
    let data = |s: &Slice| {
        set.rows[idx(s.rows.start)..idx(s.rows.end)]
            .iter()
            .filter(|r| r.kind == RowKind::Data)
            .count()
    };
    // The pages whose room is lowered, by page index, and by how much.
    let mut short: Vec<(usize, Nm)> = Vec::new();
    loop {
        let (slices, ends) = fill(set, spec, layouts, lines, &short);
        // The first short last page of a run whose page before, in the same run, can give it rows and keep as many.
        let found = (1..slices.len()).find_map(|k| {
            let (last, before) = (data(&slices[k]), data(&slices[k - 1]));
            let want = LAST_PAGE_ROWS.checked_sub(last).filter(|w| *w > 0)?;
            (ends[k]
                && !ends[k - 1]
                && before >= LAST_PAGE_ROWS + want
                && !short.iter().any(|(p, _)| *p == k - 1))
            .then_some((k - 1, want))
        });
        let Some((page, want)) = found else {
            return slices;
        };
        let height = layouts.page(idx(slices[page].sub)).data;
        short.push((
            page,
            Nm(height.0.saturating_mul(i64::try_from(want).unwrap_or(0))),
        ));
    }
}

/// The pages of a row set filled in order, each page's room lowered by `short` (by page index), and for each page
/// whether it ends a run (the last page, or the last before a group break).
fn fill(
    set: &RowSet,
    spec: &BlockSpec,
    layouts: &Layouts,
    lines: &dyn RowLines,
    short: &[(usize, Nm)],
) -> (Vec<Slice>, Vec<bool>) {
    let n = len_u32(set.rows.len());
    let groups: Vec<(Option<usize>, Range<u32>)> = if set.groups.is_empty() {
        vec![(None, 0..n)]
    } else {
        set.groups
            .iter()
            .enumerate()
            .map(|(i, g)| (Some(i), g.rows.start.min(n)..g.rows.end.min(n)))
            .collect()
    };

    let mut out: Vec<Slice> = Vec::new();
    let mut ends: Vec<bool> = Vec::new();
    let mut page = Open::new(0, 0, groups.first().map_or(0, |g| g.1.start));
    for (gi, (group, range)) in groups.iter().enumerate() {
        if range.is_empty() {
            continue;
        }
        if gi > 0 && page.has_rows() {
            match spec.group_break {
                Break::None => {}
                Break::NewPage => page = page.close_into(&mut out, Next::Page, range.start),
                Break::NewFolio => page = page.close_into(&mut out, Next::Folio, range.start),
            }
            // A page closed by the break ends its run.
            ends.resize(out.len(), true);
        }
        if !page.has_rows() {
            page.rows = range.start..range.start;
        }
        for i in range.clone() {
            let row = &set.rows[idx(i)];
            let layout = layouts.page(idx(page.sub));
            let h = row_height(row, layout, lines);
            let mut need = h;
            if matches!(row.kind, RowKind::GroupHeader | RowKind::Heading) {
                // Keep-with-next (doc 09 §3.2): the header or heading stays with at least one data row.
                for j in i + 1..range.end {
                    let next = &set.rows[idx(j)];
                    match next.kind {
                        RowKind::Data => {
                            need = Nm(need.0.saturating_add(row_height(next, layout, lines).0));
                            break;
                        }
                        RowKind::GroupHeader | RowKind::Heading => {
                            need = Nm(need.0.saturating_add(row_height(next, layout, lines).0));
                        }
                        RowKind::GroupFooter => break,
                    }
                }
            }
            let room = short
                .iter()
                .filter(|(p, _)| *p == out.len())
                .fold(layout.capacity.0, |c, (_, s)| c.saturating_sub(s.0));
            if page.used.0.saturating_add(need.0) > room && page.has_rows() {
                page = page.close_into(&mut out, Next::Page, i);
                ends.resize(out.len(), false);
                let next_layout = layouts.page(idx(page.sub));
                if group.is_some()
                    && next_layout.repeats_group_header()
                    && set.rows[idx(range.start)].kind == RowKind::GroupHeader
                    && i > range.start
                    && row.kind != RowKind::GroupHeader
                {
                    page.used = next_layout.height(RowKind::GroupHeader);
                    page.continued = true;
                    page.group = *group;
                }
                // Re-measure on the new page's form.
                page.place(i, row_height(row, next_layout, lines), *group);
            } else {
                page.place(i, h, *group);
            }
        }
    }
    if page.has_rows() {
        out.push(page.into_slice());
    }
    ends.resize(out.len(), true);
    (out, ends)
}

/// The pages of a table of contents with `toc_pages` pages of `rows_per_page` rows each (the row count is
/// clamped by `page_order`, which knows it).
pub fn toc_slices(toc_pages: u32, rows_per_page: u32) -> Vec<Slice> {
    (0..toc_pages.max(1))
        .map(|k| Slice {
            rows: k.saturating_mul(rows_per_page)..(k + 1).saturating_mul(rows_per_page),
            group: None,
            continued_group: false,
            folio: 0,
            sub: k,
            note: None,
        })
        .collect()
}

/// Numbers the slices into generated pages: folio numbers and labels, titles, forms, IDs, page count.
pub fn number(slices: &[Slice], set: &RowSet, spec: &BlockSpec, layouts: &Layouts) -> Vec<GeneratedPage> {
    let layouts = &layouts.validated();
    let count = len_u32(slices.len());
    slices
        .iter()
        .zip(0u32..)
        .map(|(s, k)| {
            let (number, label) = match spec.continuation {
                Continuation::SubPage => {
                    let number = spec.first_folio.saturating_add(s.folio);
                    let sub = s.sub.saturating_add(spec.first_sub);
                    let label = if sub == 0 {
                        number.to_string()
                    } else {
                        format!("{number}.{sub}")
                    };
                    (number, label)
                }
                Continuation::NextFolio => {
                    let number = spec.first_folio.saturating_add(k);
                    (number, number.to_string())
                }
            };
            let group = s.group.and_then(|g| set.groups.get(g)).map(|g| g.text.clone());
            let group_text = group.clone().unwrap_or_default();
            let n = (k + 1).to_string();
            let total = count.to_string();
            let title = map_ml(&spec.title, |t| fill_title(t, &n, &total, &group_text));
            GeneratedPage {
                id: v5::report_page_id(spec.block, k),
                block: spec.block,
                index: k,
                section: spec.section,
                number,
                label,
                title,
                rows: s.rows.clone(),
                group,
                continued_group: s.continued_group,
                form: layouts.page(idx(s.sub)).form,
                page_count: count,
                note: s.note.clone(),
            }
        })
        .collect()
}

/// The page being filled.
struct Open {
    rows: Range<u32>,
    used: Nm,
    group: Option<usize>,
    continued: bool,
    folio: u32,
    sub: u32,
}

impl Open {
    fn new(folio: u32, sub: u32, start: u32) -> Open {
        Open {
            rows: start..start,
            used: Nm::ZERO,
            group: None,
            continued: false,
            folio,
            sub,
        }
    }

    fn has_rows(&self) -> bool {
        !self.rows.is_empty()
    }

    fn place(&mut self, i: u32, h: Nm, group: Option<usize>) {
        self.rows.end = i + 1;
        self.used = Nm(self.used.0.saturating_add(h.0));
        if self.group.is_none() {
            self.group = group;
        }
    }

    fn into_slice(self) -> Slice {
        Slice {
            rows: self.rows,
            group: self.group,
            continued_group: self.continued,
            folio: self.folio,
            sub: self.sub,
            note: None,
        }
    }

    /// Pushes this page (if it has rows) and opens the next one at row `start`.
    fn close_into(self, out: &mut Vec<Slice>, next: Next, start: u32) -> Open {
        let (folio, sub) = match next {
            Next::Page => (self.folio, self.sub + 1),
            Next::Folio => (self.folio + 1, 0),
        };
        if self.has_rows() {
            out.push(self.into_slice());
        }
        Open::new(folio, sub, start)
    }
}

/// Where the next page goes.
#[derive(Clone, Copy)]
enum Next {
    /// The next page of the same folio run (the next form of the chain).
    Page,
    /// The first page of the next folio (the chain restarts).
    Folio,
}

fn note_slice(note: MlText) -> Slice {
    Slice {
        rows: 0..0,
        group: None,
        continued_group: false,
        folio: 0,
        sub: 0,
        note: Some(note),
    }
}

fn row_height(row: &Row, layout: &PageLayout, lines: &dyn RowLines) -> Nm {
    let n = if layout.wraps(row.kind) {
        i64::from(lines.lines(row, layout).max(1))
    } else {
        1
    };
    Nm(layout.height(row.kind).0.saturating_mul(n))
}

/// Fills a page-title format in one pass (doc 09 §2.6): `{{` is a literal `{`, `}}` a literal `}`, `{n}`,
/// `{count}` and `{group}` are replaced by the given values (inserted verbatim, never scanned again), and
/// any other `{...}` or lone brace is copied unchanged.
pub fn fill_title(format: &str, n: &str, count: &str, group: &str) -> String {
    let mut out = String::with_capacity(format.len() + 8);
    let mut rest = format;
    while let Some(pos) = rest.find(['{', '}']) {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        if let Some(r) = tail.strip_prefix("{{") {
            out.push('{');
            rest = r;
        } else if let Some(r) = tail.strip_prefix("}}") {
            out.push('}');
            rest = r;
        } else if let Some(r) = tail.strip_prefix("{n}") {
            out.push_str(n);
            rest = r;
        } else if let Some(r) = tail.strip_prefix("{count}") {
            out.push_str(count);
            rest = r;
        } else if let Some(r) = tail.strip_prefix("{group}") {
            out.push_str(group);
            rest = r;
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn idx(i: u32) -> usize {
    usize::try_from(i).unwrap_or(usize::MAX)
}
