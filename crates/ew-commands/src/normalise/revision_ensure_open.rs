// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Normaliser #18, the automatic open revision (`revision.ensure_open`; doc 15 §5.3, FR-REV-009; WP-S1.18 D3).
//!
//! When a transaction changes a content record ([`ew_schema::is_content_kind`]: every record whose file the content
//! digest covers, the project row, library copies and waivers included; not revisions, transmittals or comments)
//! and the project has a released revision but no open one, it adds the open revision that collects the edit:
//! `Revision {label: <next label>, state: open}` with no author (there is no user identity at MVP; Revision ▸ Set
//! author writes it, FT-668). The label is the label format's next label after every label in the format
//! (`settings.revision_label`, absent meaning `V{major}.{minor}`; withdrawal records repeat a label and are passed
//! over), so after V1.0 it is V2.0. The ID is `v5(latest released revision, "next")`
//! ([`ew_ids::v5::revision_next_id`]), the latest being the released revision with the greatest label in the format
//! (ties and labels out of the format by ID), so a replay of the same edit opens the same record; when that ID is
//! taken (a file edited by hand or merged), the formula is applied to the taken ID again, a bounded chain. The revision is
//! part of the edit's transaction, so undo removes both.
//!
//! A label format of a newer build has no next label: the normaliser opens nothing and leaves a notice.

use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_mltext::MlText;
use ew_schema::{Extra, Revision, RevisionState, is_content_kind};

use crate::error::CmdError;
use crate::normalise::Normaliser;
use crate::tx::TxBuilder;

/// How often the ID formula is chained when its result is taken before the normaliser gives up (with a notice).
const ID_CHAIN: usize = 16;

/// Normaliser #18.
#[derive(Clone, Copy, Debug, Default)]
pub struct RevisionEnsureOpen;

impl Normaliser for RevisionEnsureOpen {
    fn id(&self) -> &'static str {
        "revision_ensure_open"
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<bool, CmdError> {
        if !tx.changed_keys().iter().any(|(kind, _)| is_content_kind(*kind)) {
            return Ok(false);
        }
        let revisions = tx.iter::<Revision>();
        if revisions.iter().any(|r| r.is_open()) {
            return Ok(false);
        }
        let format = tx.project().settings.revision_label_format();
        let Some(latest) = revisions
            .iter()
            .filter(|r| r.state == RevisionState::Released)
            .max_by_key(|r| (format.order_key(r.label.as_str()), r.id))
            .map(Arc::clone)
        else {
            return Ok(false);
        };
        let labels = revisions
            .iter()
            .filter(|r| !r.is_withdrawal())
            .map(|r| r.label.as_str());
        let Some(label) = format.next_after(labels) else {
            tx.notice(format!(
                "no revision was opened for this edit: the revision label format {} is of a newer build, so \
                 ElektraWatt cannot propose the next label after {}; create the open revision with revision.create",
                format.pattern(),
                latest.label
            ));
            return Ok(false);
        };
        let mut id: Id<Revision> = v5::revision_next_id(latest.id);
        let mut tries = 0;
        while tx.exists::<Revision>(id) {
            tries += 1;
            if tries > ID_CHAIN {
                tx.notice(format!(
                    "no revision was opened for this edit: the IDs that follow revision {} ({}) are taken; create \
                     the open revision with revision.create",
                    latest.label, latest.id
                ));
                return Ok(false);
            }
            id = v5::revision_next_id(id);
        }
        tx.insert(Revision {
            id,
            v: 1,
            label,
            date: None,
            author: None,
            checker: None,
            description: MlText::default(),
            state: RevisionState::Open,
            page_notes: Vec::new(),
            withdraws: None,
            source_ref: None,
            reason: MlText::default(),
            release: None,
            sign_off: None,
            extra: Extra::new(),
        })?;
        Ok(true)
    }
}
