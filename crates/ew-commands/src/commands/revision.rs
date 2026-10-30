// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Revisions (doc 03 §18, doc 15 §5; FR-REV-001, FR-REV-002, FR-REV-009, FR-REV-012; FT-165, WP-S1.18 part 1):
//! `revision.create` (`CreateRevision`), `revision.set_author` (`SetRevisionAuthor`) and `revision.withdraw`
//! (`WithdrawRevision`), with the checks that `fields.set`, `edit.delete` and `page.delete` share.
//!
//! The editable members of the open revision (`label`, `date`, `description`, `page_notes`) are set with
//! `fields.set` on the `revision` kind. **The roles have one confirmed path each** (WP-S1.18 D4, FT-402):
//! `fields.set` refuses `author` and `checker` from every origin; the author of the open revision is set by
//! `revision.set_author` (`HumanOnly`) and its checker only by `revision.approve` (part 3, with the sign-off);
//! `revision.create` takes both roles for a historic released revision only. A released or withdrawn revision is
//! immutable (FR-REV-009), so `fields.set` and `edit.delete` refuse it. The state of a revision never changes
//! through these commands: Open → Released is `revision.release` (the release gate and the as-built archive,
//! part 4), and a withdrawal is a new record written by `revision.withdraw`.
//!
//! The open revision that appears with the first content edit after a release is the normaliser
//! `revision_ensure_open`'s ([`crate::normalise::RevisionEnsureOpen`], WP-S1.18 D3). Deleting a page drops the open
//! revision's note on it in the same transaction (`drop_notes_on_removed_pages`, FT-303); the notes of released
//! and withdrawn revisions stay.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::{Ident, MlText};
use ew_schema::{Date, Extra, Kind, LabelFormat, Page, PageNote, PartyRole, Revision, RevisionState};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Exposure};
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::tx::TxBuilder;

/// The rule ID of the label format and order rule (doc 15 §4.8), whose prevent half `revision.create` applies.
pub const LABEL_ORDER_RULE: &str = "EW-REL-004";

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
/// and checker are party roles (FR-PRJ-003). Rejected: a checker on an open revision (at `/checker`: the checker
/// of an open revision is written only by approval, WP-S1.18 D4, FT-402); a checker equal to the author (at
/// `/checker`, EW-INV-018); a page note on a page that does not exist (at `/page_notes/<i>/page`) or a second
/// note on one page (at `/page_notes/<i>`, EW-INV-019); a label that another revision has, other than a
/// withdrawal record (EW-INV-017); state `open` while another revision is open (EW-INV-016); state `released`
/// once a revision has a release record (the typed `release` member, FT-302): a historic release precedes
/// ElektraWatt's first release and never bypasses the release gate of `revision.release`. Prevented by
/// **EW-REL-004** (the prevent half of the label rule, doc 15 §4.8 and §5.2; its release-gate half is WP-S1.18 part
/// 3): a label that is not in the project's label format (`settings.revision_label`, absent meaning
/// `V{major}.{minor}`); for an open revision, a label that does not follow every label in the format in natural
/// order (the refusal names the next label); for a historic released revision, a label that does not precede the
/// open revision's, so the open revision stays the last in order. A label format of a newer build is not judged.
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
    /// The checker, as a party role; never the author's. A historic released revision only.
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
        if self.state == NewRevisionState::Open && self.checker.is_some() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/checker",
                "the checker of an open revision is written by approval (revision.approve), never when it is \
                 created: four eyes need one confirmed path per role (FT-402); only a historic released revision \
                 takes a checker here",
            ));
        }
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
            .find(|r| !r.is_withdrawal() && r.label == self.label)
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
            if let Some(released) = revisions.iter().find(|r| r.release.is_some()) {
                return tx.reject(format!(
                    "revision {} ({}) has a release record; a historic released revision cannot follow a \
                     release by ElektraWatt: a release goes through revision.release and its gate",
                    released.label, released.id
                ));
            }
        }
        let format = tx.project().settings.revision_label_format();
        refuse_label_out_of_order(&format, &self.label, &state, &revisions)?;
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
            reason: MlText::default(),
            release: None,
            sign_off: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Revision, id.cast())))
    }
}

/// The prevent half of EW-REL-004 (doc 15 §4.8, §5.2; FR-REV-002) for a new revision in `state` among `revisions`:
/// the label is in `format`; an open revision's label follows every label in the format; a historic released
/// revision's label precedes the open revision's, so the open revision stays the last in order (withdrawal records
/// repeat a label and are passed over; labels out of the format are the gate's findings, not a bound). A format of a
/// newer build is not judged.
fn refuse_label_out_of_order(
    format: &LabelFormat,
    label: &Ident,
    state: &RevisionState,
    revisions: &[Arc<Revision>],
) -> Result<(), CmdError> {
    if matches!(format, LabelFormat::Unknown(_)) {
        return Ok(());
    }
    let opening = *state == RevisionState::Open;
    let current = || revisions.iter().filter(|r| !r.is_withdrawal());
    let next = if opening {
        format
            .next_after(current().map(|r| r.label.as_str()))
            .map_or_else(String::new, |n| format!("; the next label is {n}"))
    } else {
        String::new()
    };
    let Some(key) = format.order_key(label.as_str()) else {
        return Err(CmdError::Prevented {
            rule: LABEL_ORDER_RULE.into(),
            finding: format!(
                "the label {label} is not in the project's revision label format {} (FR-REV-002){next}",
                format.pattern()
            ),
            keys: Vec::new(),
        });
    };
    let keyed = current().filter_map(|r| format.order_key(r.label.as_str()).map(|k| (k, r)));
    let bound = if opening {
        keyed.max_by_key(|(k, r)| (*k, r.id)).filter(|(k, _)| *k >= key)
    } else {
        keyed.filter(|(_, r)| r.is_open()).find(|(k, _)| *k <= key)
    };
    let Some((_, r)) = bound else {
        return Ok(());
    };
    let finding = if opening {
        format!(
            "the label {label} does not follow {} of revision {}: revision labels increase in natural order \
             (FR-REV-002){next}",
            r.label, r.id
        )
    } else {
        format!(
            "the label {label} does not precede {} of the open revision {}: revision labels increase in natural \
             order, the open revision last (FR-REV-002)",
            r.label, r.id
        )
    };
    Err(CmdError::Prevented {
        rule: LABEL_ORDER_RULE.into(),
        finding,
        keys: vec![(Kind::Revision, r.id.cast())],
    })
}

/// `revision.set_author` (`SetRevisionAuthor`): sets the author of the open revision (WP-S1.18 D4, FT-402), the one
/// path that writes it once the revision exists (`fields.set` refuses `author` and `checker`). The GUI's Revision ▸
/// Set author dialog is WP-S1.18 part 5.
///
/// `role` is the author's party role (FR-PRJ-003). Rejected: a revision that does not exist (at `/revision`); a
/// released or withdrawn revision, which is immutable (at `/revision`, FR-REV-009); the checker's role (at `/role`,
/// four eyes, EW-INV-018). The same author again changes nothing. **Exposure `HumanOnly`** (ADR-0008 amendment 3):
/// the author decides the four-eyes sign-off, so command logs, scripts, MCP and plugins get `CmdError::Exposure`.
/// Interactive; undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetRevisionAuthor {
    /// The open revision.
    #[schemars(with = "UuidS")]
    pub revision: Id<Revision>,
    /// The author, as a party role.
    pub role: PartyRole,
}

impl Command for SetRevisionAuthor {
    const NAME: &'static str = "revision.set_author";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::HumanOnly,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let revision = require_revision(tx, Self::NAME, self.revision)?;
        refuse_closed_revision(tx, Self::NAME, "/revision", self.revision)?;
        if revision.checker.as_ref() == Some(&self.role) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/role",
                "the author is the checker's party; four eyes need another party (D-34, FR-REV-005)",
            ));
        }
        let role = self.role.clone();
        tx.update::<Revision>(self.revision, |r| r.author = Some(role))?;
        Ok(Outcome::default())
    }
}

/// `revision.withdraw` (`WithdrawRevision`): withdraws a released revision (doc 15 §5.10, FR-REV-009; WP-S1.18 D5).
/// The CLI's `ew revision withdraw <label> --reason <text>` and the GUI's Revision ▸ Withdraw are WP-S1.18 parts 3
/// and 5.
///
/// Adds a withdrawal record `Revision {label, state: withdrawn, withdraws: <revision>, reason}` with the ID
/// `v5(revision, "withdrawal")` and the label of the revision it withdraws (EW-INV-017 lets a withdrawal record
/// repeat it); the withdrawn revision stays byte for byte as it was (EW-INV-020), and so does its archive. Rejected:
/// a revision that does not exist, or that is not released (open, a withdrawal record, a state of a newer format;
/// at `/revision`); a revision withdrawn already; an empty reason or one that is only white space (at `/reason`).
/// **Exposure `HumanOnly`** (doc 15 §5.10): command logs, scripts, MCP and plugins get `CmdError::Exposure`.
/// Interactive; undoable as one step. Outcome: the withdrawal record.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WithdrawRevision {
    /// The released revision.
    #[schemars(with = "UuidS")]
    pub revision: Id<Revision>,
    /// Why it is withdrawn.
    #[schemars(with = "MlTextS")]
    pub reason: MlText,
}

impl Command for WithdrawRevision {
    const NAME: &'static str = "revision.withdraw";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::HumanOnly,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let revision = require_revision(tx, Self::NAME, self.revision)?;
        if revision.state != RevisionState::Released {
            let what = match &revision.state {
                RevisionState::Open => "open".to_owned(),
                RevisionState::Withdrawn => "a withdrawal record".to_owned(),
                other => format!(
                    "in the state {}",
                    serde_json::to_string(other).unwrap_or_default()
                ),
            };
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/revision",
                format!(
                    "revision {} ({}) is {what}; only a released revision is withdrawn (FR-REV-009)",
                    revision.label, revision.id
                ),
            ));
        }
        let blank = |t: &str| t.trim().is_empty();
        if self.reason.is_empty()
            || self.reason.as_neutral().is_some_and(blank)
            || self
                .reason
                .languages()
                .any(|tag| self.reason.get(tag).is_none_or(blank))
        {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/reason",
                "a withdrawal needs a reason (doc 15 §5.10)",
            ));
        }
        if let Some(earlier) = tx
            .iter::<Revision>()
            .iter()
            .find(|r| r.is_withdrawal() && r.withdraws == Some(self.revision))
        {
            return tx.reject(format!(
                "revision {} ({}) is withdrawn already by the withdrawal record {}",
                revision.label, revision.id, earlier.id
            ));
        }
        let id: Id<Revision> = v5::revision_withdrawal_id(self.revision);
        tx.insert(Revision {
            id,
            v: 1,
            label: revision.label.clone(),
            date: None,
            author: None,
            checker: None,
            description: MlText::default(),
            state: RevisionState::Withdrawn,
            page_notes: Vec::new(),
            withdraws: Some(self.revision),
            source_ref: None,
            reason: self.reason.clone(),
            release: None,
            sign_off: None,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Revision, id.cast())))
    }
}

/// The revision a command names, or the refusal at `/revision`.
fn require_revision(tx: &TxBuilder<'_>, command: &str, id: Id<Revision>) -> Result<Arc<Revision>, CmdError> {
    tx.get::<Revision>(id).ok_or_else(|| {
        CmdError::invalid_params(command, "/revision", format!("revision {id} does not exist"))
    })
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

/// FT-303 (WP-S1.18 D5): drops the open revision's notes on the pages this transaction removed, so a deleted page
/// leaves no note behind in the revision that collects the current edits. The notes of released and withdrawn
/// revisions stay: they are history (EW-INV-020; a note is not a link). Called by the commands that delete pages
/// (`page.delete`, `edit.delete`) after their deletions.
pub(crate) fn drop_notes_on_removed_pages(tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
    let removed: BTreeSet<Id<Page>> = tx
        .changed_of(Kind::Page)
        .into_iter()
        .filter(|key| tx.row(key).is_none())
        .map(|key| key.1.cast())
        .collect();
    if removed.is_empty() {
        return Ok(());
    }
    let Some(open) = tx.iter::<Revision>().into_iter().find(|r| r.is_open()) else {
        return Ok(());
    };
    if open.page_notes.iter().any(|n| removed.contains(&n.page)) {
        tx.update::<Revision>(open.id, |r| {
            r.page_notes.retain(|n| !removed.contains(&n.page));
        })?;
    }
    Ok(())
}
