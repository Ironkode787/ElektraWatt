// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revisions (doc 03 §18, doc 15 §5; FR-REV-001, FR-REV-012; FT-165): `revision.create`
//! (`CreateRevision`).
//!
//! The editable members of the open revision (`label`, `date`, `author`, `checker`, `description`,
//! `page_notes`) are set with `fields.set` on the `revision` kind; a released or withdrawn revision is
//! immutable (FR-REV-009), so `fields.set` and `edit.delete` refuse it. The state of a revision never
//! changes through these commands: Open → Released is `revision.release` (the release gate, the as-built
//! archive, `HumanOnly`) and a withdrawal is a new record written by `revision.withdraw` (`HumanOnly`);
//! neither is built yet.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{Date, Extra, Kind, Page, PageNote, PartyRole, Revision, RevisionState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

/// The state of a new revision.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NewRevisionState {
    /// The revision that collects the current edits (at most one, EW-INV-016).
    #[default]
    Open,
    /// A historic released revision: released before the project came to ElektraWatt, so it has no
    /// release record and no as-built archive (Golden A's V1.0).
    Released,
}

/// One page a new revision declares as changed (FR-REV-012).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionPageNote {
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// What changed on the page.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
}

/// `revision.create` (`CreateRevision`): a project revision (doc 03 §18, FR-REV-001, FT-165): the open
/// revision that collects the current edits, or a historic released revision (released before the project
/// came to ElektraWatt: no release record, no as-built archive).
///
/// Creates `Revision {label, date, author, checker, description, state, page_notes}` with a new ID; author
/// and checker are party roles (FR-PRJ-003). Rejected: a label that another revision has, other than a
/// withdrawal record (EW-INV-017); state `open` while another revision is open (EW-INV-016); a checker
/// equal to the author (at `/checker`, EW-INV-018); a page note on a page that does not exist (at
/// `/page_notes/<i>/page`) or a second note on one page (at `/page_notes/<i>`, EW-INV-019); state
/// `released` once a revision has a release record (the `release` member): a historic release precedes
/// ElektraWatt's first release and never bypasses the release gate of `revision.release`. The label's
/// format and order (FR-REV-002) are the release-gate and prevent rule EW-REL-004's, not this command's.
/// The command never changes an existing revision. Outcome: the revision.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRevision {
    /// The label in the project's one format ("V2.0").
    #[schemars(with = "IdentS")]
    pub label: Ident,
    /// `open` (the default) or `released` (historic).
    #[serde(default)]
    pub state: NewRevisionState,
    /// The revision date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Date>,
    /// The author, as a party role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<PartyRole>,
    /// The checker, as a party role; never the author's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker: Option<PartyRole>,
    /// What the revision changes.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub description: MlText,
    /// The pages the revision declares as changed, at most one note per page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub page_notes: Vec<RevisionPageNote>,
}

impl Command for CreateRevision {
    const NAME: &'static str = "revision.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.author.is_some() && self.author == self.checker {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/checker",
                "the checker is the author's party; four eyes need another party (D-34, FR-REV-005)",
            ));
        }
        let notes: Vec<PageNote> = self
            .page_notes
            .iter()
            .map(|n| PageNote {
                page: n.page,
                description: n.description.clone(),
                extra: Extra::new(),
            })
            .collect();
        check_page_notes(tx, Self::NAME, "/page_notes", &notes)?;
        let revisions = tx.iter::<Revision>();
        if let Some(twin) = revisions
            .iter()
            .find(|r| r.state != RevisionState::Withdrawn && r.label == self.label)
        {
            return tx.reject(format!(
                "revision {} already has the label {}; a label names one revision",
                twin.id, self.label
            ));
        }
        let state = match self.state {
            NewRevisionState::Open => RevisionState::Open,
            NewRevisionState::Released => RevisionState::Released,
        };
        if state == RevisionState::Open {
            if let Some(open) = revisions.iter().find(|r| r.is_open()) {
                return tx.reject(format!(
                    "revision {} ({}) is open; at most one revision collects the current edits",
                    open.label, open.id
                ));
            }
        }
        if state == RevisionState::Released {
            // A release record is the `release` member, read from `extra` while it is untyped: this check
            // moves to the typed member when `revision.release` types the release record.
            if let Some(released) = revisions.iter().find(|r| r.extra.contains_key("release")) {
                return tx.reject(format!(
                    "revision {} ({}) has a release record; a historic released revision cannot follow a \
                     release by ElektraWatt: a release goes through revision.release and its gate",
                    released.label, released.id
                ));
            }
        }
        let id: Id<Revision> = tx.mint();
        tx.insert(Revision {
            id,
            v: 1,
            label: self.label.clone(),
            date: self.date,
            author: self.author.clone(),
            checker: self.checker.clone(),
            description: self.description.clone(),
            state,
            page_notes: notes,
            withdraws: None,
            source_ref: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Revision, id.cast())))
    }
}

/// The page notes of a revision written by `command`: every page exists, and no page has two notes
/// (EW-INV-019). `path` is the JSON Pointer of the notes in the parameters.
pub(crate) fn check_page_notes(
    tx: &TxBuilder<'_>,
    command: &str,
    path: &str,
    notes: &[PageNote],
) -> Result<(), CmdError> {
    let mut seen = BTreeSet::new();
    for (i, n) in notes.iter().enumerate() {
        if !tx.exists::<Page>(n.page) {
            return Err(CmdError::invalid_params(
                command,
                format!("{path}/{i}/page"),
                format!("page {} does not exist", n.page),
            ));
        }
        if !seen.insert(n.page) {
            return Err(CmdError::invalid_params(
                command,
                format!("{path}/{i}"),
                format!("page {} has a note already; one note per page", n.page),
            ));
        }
    }
    Ok(())
}

/// A `page_notes` value of `fields.set` (`null` resets it): the members of a note are closed, as in the
/// parameters of `revision.create`, so no unknown member reaches `PageNote.extra` through a command.
pub(crate) fn refuse_open_page_notes(command: &str, value: &serde_json::Value) -> Result<(), CmdError> {
    if value.is_null() {
        return Ok(());
    }
    serde_json::from_value::<Vec<RevisionPageNote>>(value.clone())
        .map(|_| ())
        .map_err(|e| CmdError::invalid_params(command, "/value", e.to_string()))
}

/// The refusal of an edit of a revision that is not open: released and withdrawn revisions are immutable
/// (FR-REV-009). `path` is the JSON Pointer of the target in the parameters.
pub(crate) fn refuse_closed_revision(
    tx: &TxBuilder<'_>,
    command: &str,
    path: &str,
    id: Id<Revision>,
) -> Result<(), CmdError> {
    let revision = tx.require::<Revision>(id)?;
    if revision.is_open() {
        return Ok(());
    }
    Err(CmdError::invalid_params(
        command,
        path,
        format!(
            "revision {} ({}) is not open; a released or withdrawn revision is immutable (FR-REV-009)",
            revision.label, revision.id
        ),
    ))
}
