// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The query families of the report engine: a block's arranged rows and its generated pages.

use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{ReportBlock, ReportSource};
use ew_derive::{Cycle, GeneratedPage, Policy, Q, Query, ReportPagesProvider};

use crate::arrange::{Arrangement, arrange};
use crate::kind::{Registry, kind_name};
use crate::layout::{Layouts, PageLayout};
use crate::paginate::{BlockSpec, FixedLines, is_toc, number, paginate, toc_slices};
use crate::rowset::{NotDerived, RowSet};

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
            let name = kind_name(kind).unwrap_or("unknown kind");
            return Arc::new(RowSet::not_derived(
                Vec::new(),
                NotDerived::Unsupported(name.to_owned()),
            ));
        };
        let rows = def.rows(q, block);
        if is_toc(block) || !rows.is_derived() {
            return Arc::new(rows);
        }
        let layouts = Layouts::of_form(q, block.form);
        let how = Arrangement::of_block(block, layouts.first()).with_group_context(q, block);
        Arc::new(arrange(q, rows, &how))
    }
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
    const POLICY: Policy = Policy::MemoLru;

    fn run(q: &Q, key: &(Id<ReportBlock>, u32)) -> Arc<Vec<GeneratedPage>> {
        let (id, toc_pages) = *key;
        let Some(block) = q.report_blocks().get(id) else {
            return Arc::new(Vec::new());
        };
        let Some(spec) = BlockSpec::of_block(id, block) else {
            return Arc::new(Vec::new());
        };
        let layouts = Layouts::of_form(q, block.form);
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
        Arc::new(paginate(&rows, &spec, &layouts, &FixedLines))
    }
}

/// The rows one page of a TOC block holds (doc 09 §3.1, decision D3): the TOC form's data area when the
/// form has a valid one (its capacity over its data-row height), else the profile's `toc.rows_per_page`.
/// At least 1. `page_order` uses the same value for the TOC page-count fixpoint.
pub fn toc_rows_per_page(q: &Q, block: &ReportBlock) -> u32 {
    let from_form = q
        .forms()
        .get(block.form)
        .and_then(|c| c.item.data.as_ref())
        .map(|area| PageLayout::of_area(block.form, area))
        .filter(|l| l.data.0 > 0 && l.capacity.0 >= l.data.0)
        .map(|l| u32::try_from(l.capacity.0 / l.data.0).unwrap_or(u32::MAX));
    from_form.unwrap_or(q.profile().toc.rows_per_page).max(1)
}

/// An empty, not-derived row set for a query cycle, which is recorded as a note.
fn cycle_rows(q: &Q, cycle: Cycle) -> RowSet {
    let family = cycle.family.to_owned();
    q.note_cycle(cycle);
    RowSet::not_derived(Vec::new(), NotDerived::Cycle(family))
}

/// The report engine: the [`ReportPagesProvider`] the session installs on every `Q`, so that
/// `page_order` includes the generated report pages.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReportEngine;

/// The block ID that asks the engine whether it is installed: `ew_derive::report_pages(q, ENGINE_PROBE, 0)` is
/// non-empty exactly when the `Q` carries this engine. It is a fixed UUID no record has (records are v7 or
/// v5), and it is the only read of `Q` configuration a row source may make (`Q::has_report_pages` needs
/// `&mut Q`). See `kinds::conn::ready`.
pub(crate) const ENGINE_PROBE: Id<ReportBlock> = Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(
    0xE1EC_72A7_7A11_4E55_9E00_0000_0000_0001,
));

/// What the engine answers to [`ENGINE_PROBE`]: one page that belongs to no block.
fn probe_answer() -> Vec<GeneratedPage> {
    vec![GeneratedPage {
        id: Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(0)),
        block: ENGINE_PROBE,
        index: 0,
        section: None,
        number: 0,
        label: String::new(),
        title: ew_derive::ew_mltext::MlText::default(),
        rows: 0..0,
        group: None,
        continued_group: false,
        form: Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(0)),
        page_count: 1,
        note: None,
    }]
}

/// Whether the report engine is installed in `q`, through the one public read the hook offers.
pub(crate) fn engine_installed(q: &Q) -> bool {
    !ew_derive::report_pages(q, ENGINE_PROBE, 0).is_empty()
}

impl ReportPagesProvider for ReportEngine {
    fn pages(&self, q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
        if block == ENGINE_PROBE {
            return probe_answer();
        }
        let Some(b) = q.report_blocks().get(block) else {
            return Vec::new();
        };
        let key = (block, if is_toc(b) { toc_pages } else { 0 });
        match q.try_get::<BlockPages>(key) {
            Ok(pages) => (*pages).clone(),
            // The block's pages are already being computed: a row source reached the page order from
            // inside the pagination. One placeholder page keeps the folio, never a panic.
            Err(cycle) => match BlockSpec::of_block(block, b) {
                Some(spec) => {
                    let rows = cycle_rows(q, cycle);
                    paginate(&rows, &spec, &Layouts::of_form(q, b.form), &FixedLines)
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
