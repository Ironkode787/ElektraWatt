// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Built-in report row queries, user SQL reports through the sandboxed query API of `ew-sqlview`,
//! pagination into virtual pages, and XLSX and CSV tables (doc 09 §1.3, ADR-0018).
//!
//! # Pipeline
//!
//! ```text
//! report kind (ReportKindDef::rows)  →  RowSet (typed cells, stable RowIds)       [kinds, rowset]
//!   → manual selection, filter, sort, group                                       [arrange]
//!   = report_rows(block)                                         (BlockRows, Memo)
//!   → slice into pages by the form's data area, number the folios, fill titles   [layout, paginate]
//!   = report_block_pages(block, toc_pages)                    (BlockPages, MemoLru)
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
//! page whose note gives the reason, so its folio, title and the TOC stay in place (see `README.md`).

#![forbid(unsafe_code)]

pub mod arrange;
mod kind;
pub mod kinds;
pub mod layout;
pub mod paginate;
mod queries;
pub mod render;
mod rowset;

pub use arrange::{Arrangement, Filter, FilterOp, arrange};
pub use kind::{Registry, ReportKindDef, kind_name};
pub use layout::{Layouts, PageLayout};
pub use paginate::{BlockSpec, Break, Continuation, FixedLines, RowLines, Slice, paginate};
pub use queries::{BlockPages, BlockRows, ReportEngine, engine, rows_of};
pub use render::TagClassMembers;
pub use rowset::{
    Cell, CellType, ColumnDef, GridCell, GroupRange, LinkRef, LinkTarget, MissingReason, NotDerived, QtyUnit,
    Row, RowId, RowIdParseError, RowKey, RowKind, RowSet, RowSetStatus,
};

use ew_derive::{Policy, Query};

/// Every query family of this crate with its initial policy (ARCH §6.4), in name order.
pub const FAMILIES: &[(&str, Policy)] = &[
    (BlockPages::FAMILY, BlockPages::POLICY),
    (BlockRows::FAMILY, BlockRows::POLICY),
    (TagClassMembers::FAMILY, TagClassMembers::POLICY),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_names_are_sorted_and_unknown_to_ew_derive() {
        let names: Vec<&str> = FAMILIES.iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for n in names {
            assert!(
                ew_derive::FAMILIES.iter().all(|(d, _)| *d != n),
                "family `{n}` is also an ew-derive family"
            );
        }
    }

    #[test]
    fn every_registered_kind_has_columns_and_a_schema_name() {
        let reg = Registry::builtin();
        let names: Vec<&str> = reg.names().collect();
        for n in &names {
            let k = reg.by_name(n).unwrap();
            assert_eq!(k.name(), *n);
            assert!(!k.columns().is_empty(), "kind `{n}` has no columns");
        }
        for want in [
            "toc",
            "parts_summary",
            "parts_list",
            "part_data",
            "structure_overview",
            "terminal_diagram",
            "cable_overview_diagrams",
            "wire_list",
            "potential_overview",
            "plc_overview",
            "profile_tables",
            "revision_overview",
        ] {
            assert!(names.contains(&want), "kind `{want}` is not registered");
        }
    }
}
