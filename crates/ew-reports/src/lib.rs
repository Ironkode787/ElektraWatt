// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Built-in report row queries (a user SQL report gives an empty row set marked not derived until 1.0),
//! pagination into virtual pages, and the table writers: CSV presets and an own XLSX writer (doc 09 §1.3, ADR-0018;
//! [`table`], [`csv`], [`xlsx`]).
//!
//! # Pipeline
//!
//! ```text
//! report kind (ReportKindDef::rows)  →  RowSet (typed cells, stable RowIds)       [kinds, rowset]
//!   → manual selection, filter, sort, group                                       [arrange]
//!   = report_rows(block)                                         (BlockRows, Memo)
//!   → slice into pages by the form's data area, number the folios, fill titles   [layout, paginate]
//!   = report_block_pages(block, toc_pages)                    (BlockPages, Memo)
//!   → ReportEngine: the ReportPagesProvider that page_order calls                 [queries]
//! ```
//!
//! Report pages are never stored (ADR-0018): `ew_derive::PageOrder` merges them with the drawn pages and
//! dividers, through the [`ReportPagesProvider`](ew_derive::ReportPagesProvider) hook a `Q` carries
//! (`Q::with_report_pages(ew_reports::engine())`; the session installs it). The forms engine of `ew-scene`
//! draws a generated page from its `GeneratedPage` (form, title, row range) and [`BlockRows`].
//!
//! # Not derived yet
//!
//! The connectivity kinds and the kinds without source records are registered with their columns and an
//! empty row set marked [`NotDerived`]; a page-oriented block of such a kind paginates to one placeholder
//! page whose note gives the reason, so its folio, title and the TOC stay in place.
//!
//! # Block parameters
//!
//! A block's `params` member is a top-level JSON object kept in the block's `extra` (no schema member):
//! snake_case keys, JSON scalar values. A kind declares the keys it reads through `ReportKindDef::params()` (key, type
//! `Bool` or `Choice`, default; none by default), and reads the block's values through `kind_params(block)`, one
//! value per declared key in declaration order. A value outside its kind (another JSON type, a word the choice does
//! not list, "Assembly" for "assembly") takes the default, an undeclared key is ignored, and a `params` that is not an
//! object gives every default.
//!
//! # The export by kind
//!
//! `ew-api`'s `row_export(q, &RowExport::Kind(kind), None)` (`ew export report --kind <kind>`) writes
//! `project_rows(q, kind, None)`, the kind's whole-project rows in its default order with no block arrangement, in the
//! `ew` CSV dialect. Every kind's rows on the sample project are `ew-api`'s `tests/expected/kinds/<kind>.csv`.

#![forbid(unsafe_code)]

// `TEXTS`, `KEYS`, `SOURCES`, `text!` and `key!`, before the modules that name a key.
ew_l10n::texts!(crate);

pub mod arrange;
pub mod csv;
pub mod export;
mod kind;
pub mod kinds;
pub mod layout;
pub mod paginate;
mod queries;
pub mod render;
mod rowset;
pub mod table;
pub mod words;
pub mod xlsx;

pub use arrange::{Arrangement, arrange};
pub use ew_derive::bus_word;
pub use kind::{
    EmbeddedLayout, KindParams, ParamDef, ParamKind, ParamValue, Registry, ReportKindDef, kind_params,
};
pub use kinds::{bridge_groups, cable_length_words};
pub use layout::{Layouts, PageLayout};
pub use paginate::{BlockSpec, Break, Continuation, FixedLines, RowLines, Slice, paginate};
pub use queries::{BlockPages, BlockRows, ReportEngine, arranged, engine, project_rows, rows_of};
pub use render::TagClassMembers;
pub use rowset::{
    Cell, CellType, ColumnDef, GridCell, GroupRange, LinkRef, LinkTarget, MissingReason, NotDerived, QtyUnit,
    Row, RowId, RowIdParseError, RowKey, RowKind, RowSet, RowSetStatus,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_kind_has_columns_and_a_schema_name() {
        let reg = Registry::builtin();
        for n in reg.names() {
            let k = reg.by_name(n).unwrap();
            assert_eq!(k.kind().name(), Some(n));
            assert!(!k.columns().is_empty(), "kind `{n}` has no columns");
            assert!(
                k.columns().iter().all(|c| c.weight > 0),
                "kind `{n}` has a column without printed width"
            );
        }
    }
}
