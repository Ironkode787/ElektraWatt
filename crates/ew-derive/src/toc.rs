// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The table of contents: `toc_rows()` (FR-REP-003, REF-012, REF-104, doc 09 §3.1).

use std::sync::Arc;

use ew_mltext::MlText;
use ew_rds::{Aspect, RdsSyntax};

use crate::Q;
use crate::pages::{PageOrder, PageRef, PageRefStructure, PageTitle};
use crate::query::{Policy, Query};

/// One row of the table of contents.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TocRow {
    /// The page.
    pub page: PageRef,
    /// The printed folio label.
    pub folio: String,
    /// The page title (FR-PAG-010; the generated description of a divider, FR-PAG-009).
    pub title: MlText,
    /// The page's function aspect, rendered (`=P01`); empty without one.
    pub function: String,
    /// The page's location aspect, rendered (`+J-E-2416`); empty without one.
    pub location: String,
}

/// `toc_rows()`: one row per page of the page order, in page order.
#[derive(Debug)]
pub struct TocRows;

impl Query for TocRows {
    type Key = ();
    type Value = Vec<TocRow>;
    const FAMILY: &'static str = "toc_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<Vec<TocRow>> {
        let syntax = RdsSyntax::default();
        let order = q.get::<PageOrder>(());
        let rows = order
            .entries
            .iter()
            .map(|entry| {
                let structure = q.get::<PageRefStructure>(entry.page);
                TocRow {
                    page: entry.page,
                    folio: entry.folio.clone(),
                    title: (*q.get::<PageTitle>(entry.page)).clone(),
                    function: structure.render_aspect(Aspect::Function, &syntax),
                    location: structure.render_aspect(Aspect::Location, &syntax),
                }
            })
            .collect();
        Arc::new(rows)
    }
}
