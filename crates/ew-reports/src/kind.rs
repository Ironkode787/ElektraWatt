// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report-kind registry (doc 09 §2.4): each built-in kind is a registered Rust row query with a column
//! schema.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ew_derive::ew_schema::{Page, ReportBlock, ReportKind};
use ew_derive::{DeriveNote, Q};
use serde_json::Value;

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

    /// What an **embedded** block of this kind prints: the printed columns with their weights, the box
    /// title, whether each flow column draws a header row, and the number of flow columns. The default prints every
    /// column at weight 1 under a header row, no title, one flow column ([`EmbeddedLayout::every_column`]). The
    /// layout is presentation only: the rows, the CSV and the other exports keep every column of
    /// [`columns`](ReportKindDef::columns).
    fn embedded_layout(&self) -> EmbeddedLayout {
        EmbeddedLayout::every_column(&self.columns())
    }

    /// The block parameters this kind declares: the keys of a block's `params` object it reads, each
    /// with its type and default, in declaration order. Read through [`kind_params`]. None by default.
    fn params(&self) -> &'static [ParamDef] {
        &[]
    }
}

/// One declared block parameter of a kind ([`ReportKindDef::params`], WP-S3.06 D9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParamDef {
    /// The snake_case key in the block's `params` object (`mode`).
    pub key: &'static str,
    /// The values it takes.
    pub kind: ParamKind,
    /// The value when the key is absent or its value is not one the kind takes.
    pub default: ParamValue,
}

/// The values a block parameter takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParamKind {
    /// A JSON boolean.
    Bool,
    /// A JSON string, one of these words exactly (case-sensitive).
    Choice(&'static [&'static str]),
}

/// The value of a block parameter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParamValue {
    /// A [`ParamKind::Bool`] value.
    Bool(bool),
    /// A [`ParamKind::Choice`] value: one of its words.
    Choice(&'static str),
}

/// The parameters of one block, one value per declared key in declaration order ([`kind_params`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KindParams {
    values: Vec<(&'static str, ParamValue)>,
}

impl KindParams {
    /// The value of the declared boolean `key`; `false` for a key the kind does not declare as a boolean.
    pub fn bool(&self, key: &str) -> bool {
        self.values
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, v)| *v == ParamValue::Bool(true))
    }

    /// The value of the declared choice `key`; `""` for a key the kind does not declare as a choice.
    pub fn choice(&self, key: &str) -> &'static str {
        self.values
            .iter()
            .find_map(|(k, v)| match v {
                ParamValue::Choice(c) if *k == key => Some(*c),
                _ => None,
            })
            .unwrap_or("")
    }
}

/// The parameters of `block` as its kind declares them: the block's top-level `params` member, a JSON
/// object, read key by key.
///
/// A declared key that is absent takes its default. A value outside its kind (another JSON type, a word the choice
/// does not list, `"Assembly"` for `"assembly"`) takes the default, a key the kind does not declare is ignored, and a
/// `params` that is not an object gives every default; each of these pushes one [`DeriveNote::MalformedShim`] naming
/// the block in words and the key. A block of a kind without a registered engine, or with no declared parameters and
/// no `params`, gives no values and no note.
pub fn kind_params(q: &Q, block: &ReportBlock) -> KindParams {
    let Some(def) = (match &block.source {
        ew_derive::ew_schema::ReportSource::Kind(k) => Registry::builtin().get(k),
        _ => None,
    }) else {
        return KindParams::default();
    };
    let declared = def.params();
    let defaults = || KindParams {
        values: declared.iter().map(|p| (p.key, p.default)).collect(),
    };
    let Some(params) = block.extra.get("params") else {
        return defaults();
    };
    let note = |member: String, detail: String| {
        q.note(DeriveNote::MalformedShim { member, detail });
    };
    let Some(object) = params.as_object() else {
        note(
            format!("{} params", block_words(q, block, def.name())),
            format!("expected an object of parameters, found {}", value_words(params)),
        );
        return defaults();
    };
    let mut values = Vec::with_capacity(declared.len());
    for p in declared {
        let value = match object.get(p.key) {
            None => p.default,
            Some(v) => match read_param(p, v) {
                Ok(value) => value,
                Err(detail) => {
                    note(
                        format!("{} params.{}", block_words(q, block, def.name()), p.key),
                        detail,
                    );
                    p.default
                }
            },
        };
        values.push((p.key, value));
    }
    for key in object
        .keys()
        .filter(|k| declared.iter().all(|p| p.key != k.as_str()))
    {
        note(
            format!("{} params.{}", block_words(q, block, def.name()), bounded(key)),
            format!("`{}` has no parameter of this name", def.name()),
        );
    }
    KindParams { values }
}

/// The value of `v` for the parameter `p`, or why it is not one.
fn read_param(p: &ParamDef, v: &Value) -> Result<ParamValue, String> {
    match p.kind {
        ParamKind::Bool => v
            .as_bool()
            .map(ParamValue::Bool)
            .ok_or_else(|| format!("expected true or false, found {}", value_words(v))),
        ParamKind::Choice(words) => v
            .as_str()
            .and_then(|s| words.iter().find(|w| **w == s))
            .map(|w| ParamValue::Choice(w))
            .ok_or_else(|| {
                let listed: Vec<String> = words.iter().map(|w| format!("\"{w}\"")).collect();
                let expected = match listed.split_last() {
                    Some((last, [])) => last.clone(),
                    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
                    None => "no value".to_owned(),
                };
                format!("expected {expected}, found {}", value_words(v))
            }),
    }
}

/// The longest JSON text or key a note quotes, in characters; a longer one is cut and ends in "…" (hostile input).
const QUOTED_MAX: usize = 40;

/// `text` cut to [`QUOTED_MAX`] characters.
fn bounded(text: &str) -> String {
    match text.char_indices().nth(QUOTED_MAX) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// A JSON value as a note quotes it: its compact JSON text, cut to [`QUOTED_MAX`] characters.
fn value_words(v: &Value) -> String {
    bounded(&v.to_string())
}

/// A report block in words, as a note names it: `report block 912` by its first folio; an embedded block
/// `report block part_data on page 101`; a block with neither `report block parts_summary`.
fn block_words(q: &Q, block: &ReportBlock, kind: &str) -> String {
    if let Some(embed) = &block.embedded {
        if let Some(page) = q.table::<Page>().get(embed.page) {
            return format!("report block {kind} on page {}", ew_derive::folio_label(page));
        }
        return format!("report block {kind}");
    }
    match block.first_folio {
        Some(n) => format!("report block {n}"),
        None => format!("report block {kind}"),
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
