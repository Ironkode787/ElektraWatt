// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report-kind registry (doc 09 §2.4): each built-in kind is a registered Rust row query with a column
//! schema.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ew_derive::Q;
use ew_derive::ew_schema::{ReportBlock, ReportKind};

use crate::kinds;
use crate::rowset::{ColumnDef, RowSet};

/// One built-in report kind.
///
/// `rows` returns the kind's rows in its **default order** (the order of its row query, doc 20 §14.1 for
/// the BOM), data and heading rows only; the block's filter, manual selection, sort keys and grouping are
/// applied afterwards by [`crate::arrange()`]. A kind reads the block for its own parameters (the strip of a
/// terminal diagram is the block's `manual.include`, the embedded page of a PART DATA box).
///
/// # Contract
///
/// - Pure over the tracked reads of `q` (ADR-0002 decision 1); no hash-map order, no floats.
/// - It **must not read `page_order`**: rows feed pagination, which runs inside `page_order`. Page
///   positions come from `ew_derive::DrawnPageOrder`. The table of contents is the one exception: its
///   pages are counted by the fixpoint, and its rows are read only after the page order exists.
/// - Every row has a stable [`RowId`](crate::RowId) from source entity IDs, and one cell per column.
pub trait ReportKindDef: Send + Sync {
    /// The snake_case name of doc 09 §2.4 (`"parts_summary"`), as `report-blocks.jsonl` writes it.
    fn name(&self) -> &'static str;

    /// The column schema.
    fn columns(&self) -> Vec<ColumnDef>;

    /// The rows of `block`, in the kind's default order.
    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet;

    /// What an **embedded** block of this kind prints (WP-S4.01 D16): the printed columns with their weights, the box
    /// title, whether each flow column draws a header row, and the number of flow columns. The default prints every
    /// column at weight 1 under a header row, no title, one flow column ([`EmbeddedLayout::every_column`]). The
    /// layout is presentation only: the rows, the CSV and the other exports keep every column of
    /// [`columns`](ReportKindDef::columns).
    fn embedded_layout(&self) -> EmbeddedLayout {
        EmbeddedLayout::every_column(&self.columns())
    }
}

/// How an embedded report block prints on its page ([`ReportKindDef::embedded_layout`], WP-S4.01 D16).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedLayout {
    /// The printed columns in order: `(column key, relative width)`. A key the row set does not have prints nothing.
    pub columns: Vec<(String, u32)>,
    /// The box title (neutral text); empty: no title band. A block's own non-empty `title` replaces it.
    pub title: String,
    /// Whether each flow column starts with a header row of the column labels.
    pub headers: bool,
    /// The number of flow columns the rows fill in turn (at least 1).
    pub flow_columns: u32,
}

impl EmbeddedLayout {
    /// The default layout: every column of `columns` at weight 1 under a header row, no title, one flow column. A
    /// table of a kind without a declared layout is unreadable without its column names (the orchestrator's ruling 2
    /// of cycle 42).
    pub fn every_column(columns: &[ColumnDef]) -> EmbeddedLayout {
        EmbeddedLayout {
            columns: columns.iter().map(|c| (c.key.clone(), 1)).collect(),
            title: String::new(),
            headers: true,
            flow_columns: 1,
        }
    }
}

/// The registry of built-in kinds, by name.
pub struct Registry {
    kinds: BTreeMap<&'static str, &'static dyn ReportKindDef>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.kinds.keys()).finish()
    }
}

static BUILTIN: LazyLock<Registry> = LazyLock::new(|| {
    let mut kinds = BTreeMap::new();
    for k in kinds::builtin() {
        let previous = kinds.insert(k.name(), k);
        assert!(previous.is_none(), "report kind `{}` registered twice", k.name());
    }
    Registry { kinds }
});

impl Registry {
    /// The built-in kinds of this build.
    pub fn builtin() -> &'static Registry {
        &BUILTIN
    }

    /// The kind with this name.
    pub fn by_name(&self, name: &str) -> Option<&'static dyn ReportKindDef> {
        self.kinds.get(name).copied()
    }

    /// The kind of a block source.
    pub fn get(&self, kind: &ReportKind) -> Option<&'static dyn ReportKindDef> {
        kind_name(kind).and_then(|n| self.by_name(n))
    }

    /// The registered names, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.kinds.keys().copied()
    }
}

/// The snake_case name of a schema report kind; `None` for a kind of a newer format.
pub fn kind_name(kind: &ReportKind) -> Option<&'static str> {
    Some(match kind {
        ReportKind::ProfileTables => "profile_tables",
        ReportKind::PartData => "part_data",
        ReportKind::Toc => "toc",
        ReportKind::StructureOverview => "structure_overview",
        ReportKind::PartsSummary => "parts_summary",
        ReportKind::PartsList => "parts_list",
        ReportKind::DeviceList => "device_list",
        ReportKind::TerminalDiagram => "terminal_diagram",
        ReportKind::TerminalOverview => "terminal_overview",
        ReportKind::CableOverviewDiagrams => "cable_overview_diagrams",
        ReportKind::WireList => "wire_list",
        ReportKind::PlcOverview => "plc_overview",
        ReportKind::IoList => "io_list",
        ReportKind::PotentialOverview => "potential_overview",
        ReportKind::RevisionOverview => "revision_overview",
        ReportKind::OpenItems => "open_items",
        ReportKind::NumberingReport => "numbering_report",
        ReportKind::EnclosureLegend => "enclosure_legend",
        ReportKind::CheckReport => "check_report",
        ReportKind::Unknown(_) => return None,
    })
}
