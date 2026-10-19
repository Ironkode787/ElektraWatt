// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `toc`: the table of contents (FR-REP-003; golden 2, 2.1).

use ew_derive::ew_ids::v5::report_page_id;
use ew_derive::ew_schema::{AnyId, ReportBlock};
use ew_derive::{PageOrder, PageRef, Q, TocRows};

use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, LinkRef, LinkTarget, NotDerived, Row, RowId, RowKey, RowSet};

/// The table of contents.
///
/// One row per page of the page order, in page order. Unlike every other kind it reads the page order (through
/// `TocRows`), so its rows are read only at draw and export time, never inside the pagination.
#[derive(Clone, Copy, Debug)]
pub struct Toc;

impl ReportKindDef for Toc {
    fn name(&self) -> &'static str {
        "toc"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("folio", CellType::Folio, "Folio"),
            ColumnDef::new("title", CellType::Text, "Title"),
            ColumnDef::new("function", CellType::Ident, "Function (=)"),
            ColumnDef::new("location", CellType::Ident, "Location (+)"),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        // Read inside the pagination (a row source or provider that reaches a TOC's rows from there), the
        // page order is still being computed: a cycle gives a not-derived set, never a panic.
        if let Err(cycle) = q.try_get::<PageOrder>(()) {
            let family = cycle.family.to_owned();
            q.note_cycle(cycle);
            return RowSet::not_derived(self.columns(), NotDerived::Cycle(family));
        }
        let mut set = RowSet::new(self.columns());
        for row in q.get::<TocRows>(()).iter() {
            let (id, source): (RowId, AnyId) = match row.page {
                PageRef::Drawn(p) => (RowId::entity(p), p.cast()),
                PageRef::Divider(s) => (
                    RowId(vec![
                        RowKey::Name("divider".to_owned()),
                        RowKey::Entity(s.as_uuid()),
                    ]),
                    s.cast(),
                ),
                PageRef::Generated { block, index } => (
                    RowId(vec![RowKey::Entity(
                        report_page_id::<_, ()>(block, index).as_uuid(),
                    )]),
                    block.cast(),
                ),
            };
            let mut r = Row::data(
                id,
                vec![
                    Cell::Folio(row.page),
                    Cell::Text(row.title.clone()),
                    Cell::Ident(row.function.clone()),
                    Cell::Ident(row.location.clone()),
                ],
            );
            r.links = vec![LinkRef {
                column: 0,
                target: LinkTarget::Page(row.page),
            }];
            r.sources = vec![source];
            set.rows.push(r);
        }
        set
    }
}
