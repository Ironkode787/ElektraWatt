// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Placed elements → devices, functions and representations (doc 14 §4.4–§4.6).

use std::collections::{BTreeMap, BTreeSet};

use ew_schema::Record;

use super::Def;
use super::builtins::Builtins;
use super::cx::{ElementKey, Findings, Ids, Registry};
use super::model::QetProject;
use super::pages::PagesOut;

/// The inputs of the element mapper.
pub struct Inputs<'a> {
    /// The project.
    pub project: &'a QetProject,
    /// The mapped pages.
    pub pages: &'a PagesOut,
    /// The definitions by collection path.
    pub defs: &'a BTreeMap<String, Def>,
    /// Conductor ends per element.
    pub counts: &'a BTreeMap<ElementKey, u32>,
    /// IDs.
    pub ids: &'a Ids,
    /// Built-in documents.
    pub builtins: &'a Builtins,
}

/// What the element mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ElementsOut {
    /// Device, function and representation records.
    pub records: Vec<Record>,
    /// The placed elements for the conductor mapper.
    pub registry: Registry,
    /// The collection paths of the definitions placed elements use.
    pub used_defs: BTreeSet<String>,
    /// Elements dropped (reported).
    pub dropped: u32,
    /// Objects inferred (strips).
    pub inferred: u32,
}

/// Maps the placed elements.
pub fn map(inp: &Inputs<'_>, used: &mut BTreeSet<String>, findings: &mut Findings) -> ElementsOut {
    let _ = (inp, used, findings);
    ElementsOut::default()
}
