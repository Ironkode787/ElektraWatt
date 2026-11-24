// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The query families of the report engine: a block's arranged rows and its generated pages.

use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Device, ManualSelection, ReportBlock, ReportKind, ReportSource};
use ew_derive::{
    Cycle, GeneratedPage, Policy, Q, Query, ReportPagesProvider, folio_label, map_ml, short_tag,
};

use crate::arrange::{Arrangement, arrange};
use crate::kind::Registry;
use crate::layout::{Layouts, PageLayout};
use crate::paginate::{
    BlockSpec, Continuation, FixedLines, is_toc, legend_first_sub, number, slice, toc_slices,
};
use crate::rowset::{Cell, CellType, NotDerived, Row, RowId, RowKey, RowSet};

/// `report_rows(block)`: the rows of a report block in final order, after the kind's row query, manual
/// selection, filter, sort and grouping (doc 09 §2.3, §3.3). Embedded blocks (PART DATA, cover tables)
/// read it as well as paginated ones; so will XLSX and CSV.
///
/// A missing block gives an empty set. A table of contents is not arranged: its row order is the page
/// order. A kind without a row query, a user SQL report (1.0) and a kind of a newer format give an empty
/// set marked [`NotDerived`].
///
/// Reading this for a table of contents reads `page_order`; never do so from inside the pagination.
#[derive(Debug)]
pub struct BlockRows;

impl Query for BlockRows {
    type Key = Id<ReportBlock>;
    type Value = RowSet;
    const FAMILY: &'static str = "report_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Id<ReportBlock>) -> Arc<RowSet> {
        let Some(block) = q.report_blocks().get(*key) else {
            return Arc::new(RowSet::new(Vec::new()));
        };
        let kind = match &block.source {
            ReportSource::Kind(k) => k,
            ReportSource::SqlQuery(_) => {
                return Arc::new(RowSet::not_derived(
                    Vec::new(),
                    NotDerived::Unsupported("sql_query".to_owned()),
                ));
            }
            ReportSource::Unknown(_) => {
                return Arc::new(RowSet::not_derived(
                    Vec::new(),
                    NotDerived::Unsupported("unknown source".to_owned()),
                ));
            }
        };
        let Some(def) = Registry::builtin().get(kind) else {
            let name = kind.name().unwrap_or("unknown kind");
            return Arc::new(RowSet::not_derived(
                Vec::new(),
                NotDerived::Unsupported(name.to_owned()),
            ));
        };
        let rows = def.rows(q, block);
        if is_toc(block) || !rows.is_derived() {
            return Arc::new(rows);
        }
        Arc::new(arranged(q, block, with_added_rows(rows, block)))
    }
}

/// `rows` with the block's added rows after them (doc 09 §3.8): each a data row of its texts by column key (a column
/// it does not name empty), keyed `n:added/i:<k>` by its place in the list, linking nowhere. A text in a column of
/// identifiers or addresses is one of them, a whole number in a column of numbers is that number, so that the block's
/// sorting places the row among the model's rows; every other text is a text cell.
fn with_added_rows(mut rows: RowSet, block: &ReportBlock) -> RowSet {
    for (k, added) in block.added_rows.iter().enumerate() {
        let cells = rows
            .columns
            .iter()
            .map(|c| {
                let Some(text) = added.cells.get(&c.key).filter(|t| !t.is_empty()) else {
                    return Cell::Empty;
                };
                let plain = ew_derive::text_in(text, None).trim().to_owned();
                match c.ty {
                    CellType::Ident => Cell::Ident(plain),
                    CellType::Address => Cell::Address(plain),
                    CellType::Number => plain
                        .parse()
                        .map_or_else(|_| Cell::Text(text.clone()), Cell::Number),
                    _ => Cell::Text(text.clone()),
                }
            })
            .collect();
        let id = RowId(vec![
            RowKey::Name("added".to_owned()),
            RowKey::Index(u32::try_from(k).unwrap_or(u32::MAX)),
        ]);
        rows.rows.push(Row::data(id, cells));
    }
    rows
}

/// `rows` after the block's manual selection, filter, sort and grouping: the tail of [`BlockRows`], shared
/// with the exports that bring their own rows (`ew export bom --mode`).
pub fn arranged(q: &Q, block: &ReportBlock, rows: RowSet) -> RowSet {
    let layouts = Layouts::of_block(q, block);
    let how = Arrangement::of_block(block, layouts.first()).with_group_context(q, block);
    arrange(q, rows, &how)
}

/// `report_block_pages(block, toc_pages)`: the generated pages of a page-oriented block (doc 09 §3.2).
///
/// The key's `toc_pages` matters only for a table of contents (its page count is the `page_order`
/// fixpoint's); other blocks are asked with 0. An embedded block, and one without a first folio, has no
/// pages. Never reads `page_order`.
#[derive(Debug)]
pub struct BlockPages;

impl Query for BlockPages {
    type Key = (Id<ReportBlock>, u32);
    type Value = Vec<GeneratedPage>;
    const FAMILY: &'static str = "report_block_pages";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &(Id<ReportBlock>, u32)) -> Arc<Vec<GeneratedPage>> {
        let (id, toc_pages) = *key;
        let Some(block) = q.report_blocks().get(id) else {
            return Arc::new(Vec::new());
        };
        let Some(mut spec) = spec_of(q, id, block) else {
            return Arc::new(Vec::new());
        };
        spec.title = resolve_title(q, block, &spec.title);
        let layouts = Layouts::of_block(q, block);
        if is_toc(block) {
            let slices = toc_slices(toc_pages, toc_rows_per_page(q, block));
            return Arc::new(number(&slices, &RowSet::new(Vec::new()), &spec, &layouts));
        }
        // A row source that reads the page order (or these pages) from inside the pagination is a cycle:
        // the block gets one placeholder page instead of a panic of the whole page order.
        let rows = match q.try_get::<BlockRows>(id) {
            Ok(rows) => rows,
            Err(cycle) => Arc::new(cycle_rows(q, cycle)),
        };
        Arc::new(paginate_block(q, block, spec, &rows, &layouts))
    }
}

/// The pagination parameters of `block` ([`BlockSpec::of_block`]); a legend list (`layout_page`, doc 09 §3.2) takes
/// the folio number and section of the layout page it follows (the first that shows its plate,
/// `ew_derive::layout::legend_layout_page`), its pages sub-pages of it. A legend whose layout page is gone has none.
fn spec_of(q: &Q, id: Id<ReportBlock>, block: &ReportBlock) -> Option<BlockSpec> {
    let mut spec = BlockSpec::of_block(id, block)?;
    if block.layout_page.is_some() {
        let page = ew_derive::layout::legend_layout_page(q, block).and_then(|p| q.pages().get(p))?;
        spec.first_folio = page.folio.number;
        spec.section = page.folio.section;
        spec.continuation = Continuation::SubPage;
    }
    Some(spec)
}

/// [`paginate`] of `rows`, a legend list's pages numbered after its layout page at the first run of sub-pages no drawn
/// page of its section holds ([`legend_first_sub`]).
fn paginate_block(
    q: &Q,
    block: &ReportBlock,
    mut spec: BlockSpec,
    rows: &RowSet,
    layouts: &Layouts,
) -> Vec<GeneratedPage> {
    let slices = slice(rows, &spec, layouts, &FixedLines);
    if let Some(layout) = ew_derive::layout::legend_layout_page(q, block).and_then(|p| q.pages().get(p)) {
        let held: Vec<String> = q
            .pages()
            .iter()
            .filter(|(_, p)| p.folio.section == spec.section)
            .map(|(_, p)| folio_label(p))
            .collect();
        let count = u32::try_from(slices.len()).unwrap_or(u32::MAX);
        spec.first_sub = legend_first_sub(spec.first_folio, &folio_label(layout), &held, count);
    }
    number(&slices, rows, &spec, layouts)
}

/// The page-title format with `{strip}`, `{device}` and `{layout}` resolved: the short tag of the first device of the
/// block's manual selection (the first strip for `{strip}`), so that renaming or renumbering the device renames the
/// pages ("Terminal Diagram {strip}"), and the folio of the layout page a legend list follows ("Legend of layout
/// {layout}"). A
/// placeholder without such a device or page stays as written.
fn resolve_title(
    q: &Q,
    block: &ReportBlock,
    title: &ew_derive::ew_mltext::MlText,
) -> ew_derive::ew_mltext::MlText {
    let devices: Vec<Id<Device>> = block
        .manual
        .include
        .as_slice()
        .iter()
        .map(|a| Id::<Device>::from_uuid(a.as_uuid()))
        .filter(|d| q.devices().contains(*d))
        .collect();
    let tag = |d: Id<Device>| short_tag(q, d);
    let strip = devices
        .iter()
        .find(|d| q.devices().get(**d).is_some_and(|x| x.aspects.strip.is_some()))
        .map(|d| tag(*d));
    let device = devices.first().map(|d| tag(*d));
    let layout = ew_derive::layout::legend_layout_page(q, block)
        .and_then(|p| q.pages().get(p))
        .map(|p| folio_label(p));
    map_ml(title, |t| {
        let mut t = t.to_owned();
        if let Some(s) = &strip {
            t = t.replace("{strip}", s);
        }
        if let Some(d) = &device {
            t = t.replace("{device}", d);
        }
        if let Some(l) = &layout {
            t = t.replace("{layout}", l);
        }
        t
    })
}

/// The rows one page of a TOC block holds (doc 09 §3.1, decision D3), on the frame its pages print on
/// ([`crate::layout::list_frame`]): the form's data area when it has a valid one (its capacity over its data-row
/// height), else the rows its list area holds with the block's caption and legend (doc 09 §3.8), else (a form the
/// project does not hold) the profile's `toc.rows_per_page`. At least 1. `page_order` uses the same value for the TOC
/// page-count fixpoint.
pub fn toc_rows_per_page(q: &Q, block: &ReportBlock) -> u32 {
    let form = crate::layout::list_frame(q, block.form);
    let from_form = q.forms().get(form).and_then(|c| match c.item.data.as_ref() {
        Some(area) => Some(PageLayout::of_area(form, area))
            .filter(|l| l.data.0 > 0 && l.capacity.0 >= l.data.0)
            .map(|l| l.capacity.0 / l.data.0),
        None => Some(crate::layout::list_rows(
            q,
            &c.item,
            !block.caption.is_empty(),
            !block.legend.is_empty(),
        )),
    });
    from_form
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
        .unwrap_or(q.profile().toc.rows_per_page)
        .max(1)
}

/// An empty, not-derived row set for a query cycle, which is recorded as a note.
pub(crate) fn cycle_rows(q: &Q, cycle: Cycle) -> RowSet {
    let family = cycle.family.to_owned();
    q.note_cycle(cycle);
    RowSet::not_derived(Vec::new(), NotDerived::Cycle(family))
}

/// The report engine: the [`ReportPagesProvider`] the session installs on every `Q`, so that
/// `page_order` includes the generated report pages.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReportEngine;

impl ReportPagesProvider for ReportEngine {
    fn pages(&self, q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
        let Some(b) = q.report_blocks().get(block) else {
            return Vec::new();
        };
        let key = (block, if is_toc(b) { toc_pages } else { 0 });
        match q.try_get::<BlockPages>(key) {
            Ok(pages) => (*pages).clone(),
            // The block's pages are already being computed: a row source reached the page order from
            // inside the pagination. One placeholder page keeps the folio, never a panic.
            Err(cycle) => match spec_of(q, block, b) {
                Some(spec) => {
                    let rows = cycle_rows(q, cycle);
                    paginate_block(q, b, spec, &rows, &Layouts::of_block(q, b))
                }
                None => {
                    q.note_cycle(cycle);
                    Vec::new()
                }
            },
        }
    }

    fn toc_rows_per_page(&self, q: &Q, block: Id<ReportBlock>) -> u32 {
        match q.report_blocks().get(block) {
            Some(b) => toc_rows_per_page(q, b),
            None => q.profile().toc.rows_per_page.max(1),
        }
    }
}

/// The report engine as a provider handle, for `Q::with_report_pages`.
pub fn engine() -> Arc<dyn ReportPagesProvider> {
    Arc::new(ReportEngine)
}

/// The arranged rows of a block (`q.get::<BlockRows>(block)`).
pub fn rows_of(q: &Q, block: Id<ReportBlock>) -> Arc<RowSet> {
    q.get::<BlockRows>(block)
}

/// The rows of the built-in report kind `kind` for the whole project (D3): the rows a block of that kind
/// would list with no manual selection, no filter, no sort keys, no grouping and no page limits, in the kind's default
/// order; for `terminal_diagram` the rows of the strip `include`. The navigators read their tables from it, so a
/// printed list and its navigator never disagree.
///
/// `None` for an unknown kind, and for `terminal_diagram` without `include`. Not memoised (the navigator fetches it once
/// per revision for an open navigator only); it reads what the kind's row query reads and never `page_order`.
pub fn project_rows(q: &Q, kind: &ReportKind, include: Option<ew_derive::ew_ids::Uuid>) -> Option<RowSet> {
    let def = Registry::builtin().get(kind)?;
    let mut manual = ManualSelection::default();
    match include {
        Some(u) => {
            manual.include.insert(Id::from_uuid(u));
        }
        None if *kind == ReportKind::TerminalDiagram => return None,
        None => {}
    }
    let block = ReportBlock {
        id: Id::from_uuid(ew_derive::ew_ids::Uuid::nil()),
        v: 1,
        source: ReportSource::Kind(kind.clone()),
        manual,
        sort: Vec::new(),
        group: None,
        form: Id::from_uuid(ew_derive::ew_ids::Uuid::nil()),
        section: None,
        first_folio: None,
        continuation: Default::default(),
        title: Default::default(),
        empty: Default::default(),
        embedded: None,
        source_ref: None,
        extra: Default::default(),
        columns: Vec::new(),
        caption: Default::default(),
        legend: Default::default(),
        added_rows: Vec::new(),
        layout_page: None,
    };
    Some(def.rows(q, &block))
}
