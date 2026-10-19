// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The query families of the report engine: a block's arranged rows and its generated pages.

use std::sync::Arc;

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{ReportBlock, ReportSource};
use ew_derive::{GeneratedPage, Policy, Q, Query, ReportPagesProvider};

use crate::arrange::{Arrangement, arrange};
use crate::kind::{Registry, kind_name};
use crate::layout::Layouts;
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
        Arc::new(arrange(q, rows, &Arrangement::of_block(block, layouts.first())))
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
            let per_page = q.profile().toc.rows_per_page.max(1);
            let slices = toc_slices(toc_pages, per_page);
            return Arc::new(number(&slices, &RowSet::new(Vec::new()), &spec, &layouts));
        }
        let rows = q.get::<BlockRows>(id);
        Arc::new(paginate(&rows, &spec, &layouts, &FixedLines))
    }
}

/// The report engine: the [`ReportPagesProvider`] the session installs on every `Q`, so that
/// `page_order` includes the generated report pages.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReportEngine;

impl ReportPagesProvider for ReportEngine {
    fn pages(&self, q: &Q, block: Id<ReportBlock>, toc_pages: u32) -> Vec<GeneratedPage> {
        let toc = q.report_blocks().get(block).is_some_and(|b| is_toc(b));
        (*q.get::<BlockPages>((block, if toc { toc_pages } else { 0 }))).clone()
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
