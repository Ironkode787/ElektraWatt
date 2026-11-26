// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report-kind registry (doc 09 §2.4): each built-in kind is a registered Rust row query with a column
//! schema.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ew_derive::Q;
use ew_mltext::MlText;
use ew_schema::{ReportBlock, ReportKind};
use serde_json::Value;

use crate::kinds;
use crate::rowset::{ColumnDef, RowSet};

/// One built-in report kind.
///
/// `rows` returns the kind's rows in its **default order** (the order of its row query, for
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
    /// The kind (doc 09 §2.4); its stored spelling (`parts_summary`) is its name ([`ReportKind::name`]).
    fn kind(&self) -> ReportKind;

    /// The column schema.
    fn columns(&self) -> Vec<ColumnDef>;

    /// The rows of `block`, in the kind's default order.
    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet;

    /// What an **embedded** block of this kind prints: the printed columns with their weights, the box
    /// title, whether each flow column draws a header row, and the number of flow columns. The default prints the
    /// columns a page block prints ([`printed`](ReportKindDef::printed), else every column) at weight 1 under a header
    /// row, no title, one flow column ([`EmbeddedLayout::every_column`]). The layout is presentation only: the rows,
    /// the CSV and the other exports keep every column of [`columns`](ReportKindDef::columns).
    fn embedded_layout(&self) -> EmbeddedLayout {
        let columns = self.columns();
        match self.printed() {
            Some(keys) => {
                let printed: Vec<ColumnDef> = keys
                    .iter()
                    .filter_map(|k| columns.iter().find(|c| c.key == *k).cloned())
                    .collect();
                EmbeddedLayout::every_column(&printed)
            }
            None => EmbeddedLayout::every_column(&columns),
        }
    }

    /// The columns a page block of this kind prints when the block chooses none (`ReportBlock.columns` empty, doc 09
    /// §3.8), by key in their printed order, each at its column's weight; `None` (the default): every column. The rows,
    /// the CSV and the other exports keep every column of [`columns`](ReportKindDef::columns).
    fn printed(&self) -> Option<&'static [&'static str]> {
        None
    }

    /// The block parameters this kind declares: the keys of a block's `params` object it reads, each
    /// with its type and default, in declaration order. Read through [`kind_params`]. None by default.
    fn params(&self) -> &'static [ParamDef] {
        &[]
    }

    /// Whether a block of this kind fills pages of its own (Output ▸ Reports ▸ Report block… offers it); `false` for a
    /// kind that is only embedded on a drawn page (the profile tables, the PART DATA box).
    fn page_oriented(&self) -> bool {
        true
    }

    /// Whether Output ▸ Reports ▸ Report block… offers this kind: a kind whose blocks fill pages of their own
    /// ([`page_oriented`](ReportKindDef::page_oriented)), but not one a newer kind took over, whose stored blocks still
    /// print (`tube_cut_list`, the cut list's Tubes group).
    fn offered(&self) -> bool {
        self.page_oriented()
    }
}

/// One declared block parameter of a kind ([`ReportKindDef::params`]).
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
/// `params` that is not an object gives every default. A block of a kind without a registered engine, or with no
/// declared parameters and no `params`, gives no values.
pub fn kind_params(block: &ReportBlock) -> KindParams {
    let Some(def) = (match &block.source {
        ew_schema::ReportSource::Kind(k) => Registry::builtin().get(k),
        _ => None,
    }) else {
        return KindParams::default();
    };
    let object = block.extra.get("params").and_then(Value::as_object);
    let values = def
        .params()
        .iter()
        .map(|p| {
            let value = object
                .and_then(|o| o.get(p.key))
                .and_then(|v| read_param(p, v))
                .unwrap_or(p.default);
            (p.key, value)
        })
        .collect();
    KindParams { values }
}

/// The value of `v` for the parameter `p`; `None` for a value outside its kind.
fn read_param(p: &ParamDef, v: &Value) -> Option<ParamValue> {
    match p.kind {
        ParamKind::Bool => v.as_bool().map(ParamValue::Bool),
        ParamKind::Choice(words) => v
            .as_str()
            .and_then(|s| words.iter().find(|w| **w == s))
            .map(|w| ParamValue::Choice(w)),
    }
}

/// How an embedded report block prints on its page ([`ReportKindDef::embedded_layout`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedLayout {
    /// The printed columns in order: `(column key, relative width)`. A key the row set does not have prints nothing.
    pub columns: Vec<(String, u32)>,
    /// The box title, in every language of the list texts (it prints in the project's language); empty: no title
    /// band. A block's own non-empty `title` replaces it.
    pub title: MlText,
    /// Whether each flow column starts with a header row of the column labels.
    pub headers: bool,
    /// The number of flow columns the rows fill in turn (at least 1).
    pub flow_columns: u32,
}

impl EmbeddedLayout {
    /// The default layout: every column of `columns` at weight 1 under a header row, no title, one flow column. A
    /// table of a kind without a declared layout is unreadable without its column names.
    pub fn every_column(columns: &[ColumnDef]) -> EmbeddedLayout {
        EmbeddedLayout {
            columns: columns.iter().map(|c| (c.key.clone(), 1)).collect(),
            title: MlText::default(),
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
        let name = k.kind().name().expect("a built-in kind is a kind of this build");
        let previous = kinds.insert(name, k);
        assert!(previous.is_none(), "report kind `{name}` registered twice");
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
        kind.name().and_then(|n| self.by_name(n))
    }

    /// The registered names, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.kinds.keys().copied()
    }
}
