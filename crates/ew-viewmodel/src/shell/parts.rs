// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File, Import, Parts table... (FT-410): the headless action `parts.import`.
//!
//! The action asks for a `parts.csv` (the window's file dialog, or the `path` parameter), reads it with the tables
//! beside it through `ew_api::import::read_parts_tables`, and previews the `parts.import` command through the host.
//! The command's cost is Bulk, so the inline preview card always shows: Apply on a fresh card commits one transaction
//! (one undo step) and leaves the banner "Imported 29 parts from parts.csv (0 skipped)."; a stale card previews the
//! same copies again and shows a fresh card that needs a second Apply. A refused table shows its first problem, with
//! its pointer, in the banner and opens no card. The registry's `writable` check has run before the action body
//! (`ShellModel::dispatch`).

use ew_api::import::{PartsImport, parts_import_counts, read_parts_tables};
use ew_api::{AnyCommand, Origin, Submit};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Mode, Outcome, Prompt, ShellModel, string_param};
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

/// The name of the command the action dispatches.
const COMMAND: &str = "parts.import";

impl ShellModel {
    /// Whether the card on show is an import card (a window of its own, which needs neither the grid nor the find
    /// pane).
    pub fn import_card_open(&self) -> bool {
        self.has_project()
            && self
                .props
                .card()
                .is_some_and(|c| matches!(c.origin, CardOrigin::Import { .. }))
    }

    /// `parts.import`: read the table, preview the command, show the card.
    pub(super) fn act_parts_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("parts.import", Prompt::OpenPartsTable, "path", p),
        };
        let Some(project_id) = self
            .project
            .as_ref()
            .map(|p| p.session.query().project().id.as_uuid())
        else {
            return Outcome::Done;
        };
        let read = match read_parts_tables(&path, project_id) {
            Ok(read) => read,
            Err(e) => {
                let message = strings::format("error-parts-import", &[&e.to_string()]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        let params = read.params();
        let PartsImport { source, copies, .. } = read;
        self.preview_import(params, source, copies.len())
    }

    /// Previews `parts.import` with `params` (a Bulk command: always through the host, never committed directly) and
    /// shows the card.
    fn preview_import(&mut self, params: Value, source: String, copies: usize) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: COMMAND.to_owned(),
            params: params.clone(),
        };
        let submit = match self.mode {
            Mode::Gui => Submit::from(Origin::Gui),
            Mode::Headless => Submit::from(Origin::CmdLog {
                file: self.origin_file.clone(),
            }),
        };
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        bump(calls, |c| c.preview_with += 1);
        let result = project.session.preview_with(&cmd, submit);
        let derived = match &result {
            Ok(p) => project.session.derived_diff(p),
            Err(_) => Default::default(),
        };
        let (created, skipped) = result
            .as_ref()
            .ok()
            .and_then(|p| parts_import_counts(p.outcome().note.as_deref()?))
            .unwrap_or((0, 0));
        let result = result.map_err(|e| e.to_string());
        let edit = PendingEdit {
            row: String::new(),
            kind: "part".to_owned(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: COMMAND.to_owned(),
            params: params.clone(),
            typed: String::new(),
            label: source.clone(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: COMMAND.to_owned(),
                result,
                derived,
            },
            edit,
            targets: copies,
            changes: created,
            reason: "reason-parts-import",
            origin: CardOrigin::Import {
                source,
                created,
                skipped,
            },
        });
        Outcome::Done
    }

    /// Apply on an import card: a fresh card commits one transaction; a stale one previews the same copies again and
    /// shows a fresh card (a second Apply).
    pub(super) fn apply_import_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let CardOrigin::Import {
            source,
            created,
            skipped,
        } = card.origin.clone()
        else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            let outcome = self.commit_kept(preview, &edit.name, &edit.params, None);
            if outcome == Outcome::Done {
                self.banner = Some(strings::format(
                    "banner-parts-imported",
                    &[&created.to_string(), &skipped.to_string(), &source],
                ));
            }
            return outcome;
        }
        // Stale: the same copies, previewed again on the current revision.
        self.preview_import(card.edit.params, source, card.targets)
    }
}
