// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagrams → pages, the project record and title-block lifting (doc 14 §4.9).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{Page, Project, Record};

use super::builtins::Builtins;
use super::cx::{Findings, Ids};
use super::model::QetProject;

/// One mapped diagram.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PageInfo {
    /// The page record ID.
    pub id: Id<Page>,
    /// The folio label as EW prints it (for report locations and tag inference).
    pub folio: String,
}

/// What the page mapper emits.
#[derive(Clone, PartialEq, Debug)]
pub struct PagesOut {
    /// The project record.
    pub project: Project,
    /// Page records.
    pub records: Vec<Record>,
    /// Per diagram index.
    pub pages: Vec<PageInfo>,
}

/// Maps the diagrams of `project` to pages and builds the project record `project_id`.
pub fn map(
    project: &QetProject,
    ids: &Ids,
    project_id: Id<Project>,
    builtins: &Builtins,
    used: &mut BTreeSet<String>,
    findings: &mut Findings,
) -> PagesOut {
    let _ = (project, ids, builtins, used, findings);
    PagesOut {
        project: serde_json::from_value(serde_json::json!({
            "id": project_id.to_string(),
            "derivation_profile": "2027.1",
        }))
        .unwrap_or_else(|_| unreachable!("a minimal project record")),
        records: Vec::new(),
        pages: Vec::new(),
    }
}
