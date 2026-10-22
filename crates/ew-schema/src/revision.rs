// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Revisions (doc 03 §18, doc 15 §5; FR-REV-001, FR-REV-012): the `revision` record (rank 60,
//! `revisions.jsonl`).
//!
//! Implemented members are the two cases of FT-165: an open revision with label, date and state, and a
//! historic released revision without an archive (no release record), and on a withdrawal record the ID
//! of the revision it withdraws (`withdraws: RevisionId?`, doc 03 §18). The other members of doc 03 §18 (`purpose`, `sign_off`, `release`, and on a withdrawal record `reason`)
//! arrive with the commands that write them (`revision.approve`, `revision.release`,
//! `revision.withdraw`); until then they stay in `extra`, so a record written by a newer build keeps them
//! through a load and a save. Typing the reserved kind is additive within format 0.1 (ADR-0011 amendment 4,
//! decision log ID-334).

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_v1, v1};
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::track::open_enum;
use crate::value::{Date, SourceRef};
use crate::{Page, PartyRole};

/// One managed project revision (doc 03 §18, rank 60, `revisions.jsonl`; FR-REV-001).
///
/// The title block's "Revision" cell prints the current revision: the open one if there is one, else the
/// latest released one (D-05, FR-REV-003). A released revision is immutable (FR-REV-009); one without a
/// release record is historic (released outside EW, so it has no as-built archive and no page digests).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Revision {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Revision>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The label in the project's one format ("V2.0"; FR-REV-002).
    #[schemars(with = "IdentS")]
    pub label: Ident,
    /// The revision date; an open revision may have none yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// The author, as the role of a project party (doc 03 `Party`: form cells and revisions refer to
    /// parties by role, FR-PRJ-003).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<PartyRole>,
    /// The checker, as the role of a project party; never the author's role (D-34, FR-REV-005).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<PartyRole>,
    /// What the revision changes ("Changes on 101 and 106").
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// The state.
    pub state: RevisionState,
    /// The pages the revision declares as changed, each with an optional description (FR-REV-012), in
    /// authored order, at most one note per page. They decide the changed pages of a release whose
    /// predecessor has no page digests (doc 15 §5.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_notes: Vec<PageNote>,
    /// On a withdrawal record: the released revision it withdraws (doc 03 §18, FR-REV-009). The withdrawn
    /// record itself stays as it was (EW-INV-020); derivations leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub withdraws: Option<Id<Revision>>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members (`purpose`, `sign_off`, `release`, `reason` until they are typed) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Revision {
    /// Whether the revision is the open one.
    pub fn is_open(&self) -> bool {
        self.state == RevisionState::Open
    }
}

/// The stored state of a revision (doc 03 §4.2, doc 15 §5.1). "Signed off" is not a state: it is an open
/// revision whose sign-off matches the current content digest.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "RevisionState")]
pub enum RevisionState {
    /// The revision that collects the current edits; at most one (EW-INV-016).
    Open,
    /// Released: by `revision.release`, or historic (released before the project came to EW). Immutable.
    Released,
    /// A withdrawal record: it names the released revision it withdraws (`withdraws`, FR-REV-009).
    Withdrawn,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One page a revision declares as changed (FR-REV-012).
///
/// `page` is not a link (`ew-model` lists it among the fields that are not links): a released revision
/// is immutable, so its notes keep naming a page that a later revision deletes, and a note on a page that
/// does not exist selects nothing.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PageNote {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// What changed on the page.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

open_enum!(RevisionState);
