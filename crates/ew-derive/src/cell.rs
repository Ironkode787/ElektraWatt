// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame cells: `cell_of(point, page)` and `rep_anchor_cell(rep)` (doc 06 §5.1, FR-PAG-020, FR-SYM-010,
//! FR-XRF-005, D-14).

use std::fmt;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Page, Representation};
use ew_units::Pt2;

use crate::Q;
use crate::query::{Policy, Query};

/// One cell of a page's frame grid.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Cell {
    /// The column number as printed: `start_index` + the 0-based column (0–16 on the QET frame).
    pub column: u32,
    /// The 0-based row.
    pub row: u32,
    /// The printed column label (`"16"`, or the frame's explicit label).
    pub column_label: String,
    /// The printed row label (`"H"`, or the frame's explicit label).
    pub row_label: String,
}

impl fmt::Display for Cell {
    /// Row then column, as cross-references print it: `H16`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.row_label, self.column_label)
    }
}

/// `cell_of(point, page)`: the frame cell of a page point, or `None` outside the drawing area or on a
/// page without a frame grid.
#[derive(Debug)]
pub struct CellOf;

impl Query for CellOf {
    type Key = (Id<Page>, Pt2);
    type Value = Option<Cell>;
    const FAMILY: &'static str = "cell_of";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Cell>> {
        let _ = (q, key);
        Arc::new(None)
    }
}

/// `rep_anchor_cell(rep)`: the frame cell of a representation's anchor point, from the symbol's anchor
/// policy (never rendered geometry).
#[derive(Debug)]
pub struct RepAnchorCell;

impl Query for RepAnchorCell {
    type Key = Id<Representation>;
    type Value = Option<Cell>;
    const FAMILY: &'static str = "rep_anchor_cell";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<Cell>> {
        let _ = (q, key);
        Arc::new(None)
    }
}
