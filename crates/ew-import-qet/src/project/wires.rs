// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conductors → connections and junctions (doc 14 §4.7) (STUB: implemented by the conductor work item).

use std::collections::BTreeMap;

use ew_schema::{Connection, Junction};

use super::cx::{ElementKey, Findings, Ids, Registry};
use super::model::QetProject;

/// What the conductor mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct WireOut {
    /// The connections, in diagram then conductor document order.
    pub connections: Vec<Connection>,
    /// The junctions, one per [`super::cx::Target::Junction`] element, in registry key order.
    pub junctions: Vec<Junction>,
    /// Conductors dropped because an end did not resolve or both ends resolved to the same CP.
    pub lost: u32,
}

/// The number of conductor ends attached to each placed element, by element key, before mapping: the
/// element mapper turns a junction element (one terminal, at least three conductors) into a junction.
pub fn conductor_counts(project: &QetProject) -> BTreeMap<ElementKey, u32> {
    let _ = project;
    BTreeMap::new()
}

/// Maps every conductor of `project` against the placed elements of `reg`.
pub fn map_conductors(project: &QetProject, reg: &Registry, ids: &Ids, findings: &mut Findings) -> WireOut {
    let _ = (project, reg, ids, findings);
    WireOut::default()
}
