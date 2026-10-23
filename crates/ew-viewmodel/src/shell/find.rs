// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The find pane in the shell (WP-S2.05 part 2, spec §7.3 to §7.6): the query as typed, `find.run` (one
//! `Session::find` call; typing makes none), the result cursor and the jump, and the replace through the inline
//! preview card.
//!
//! **Replace** (§7.4): `find.replace_all` re-runs the search first when the results are stale or were made for
//! another query, then previews the results' `fields.set_many` through the host and always shows the card. Apply on a
//! fresh card commits one transaction (one undo step) and leaves the note "Replaced 20; run again to refresh"; the
//! results are not re-run. **A stale replace card** is never applied with its old parameters: Apply runs the search
//! again with the card's query (one `find` call), rebuilds the command from the new hits and shows a fresh card that
//! needs a second Apply, with the note "Preview updated: 2 fewer results".

use std::sync::Arc;

use ew_api::output::ew_dl::EntityRef;
use ew_api::{AnyCommand, Exposure, Origin, Submit};
use serde_json::{Map, Value};

use super::navigation::{Focus, bump};
use super::{Mode, Outcome, ShellModel, failed, string_param};
use crate::find::{FindQuery, FindResults, FindState, Ran, field_of_word, updated_note};
use crate::layout::PaneKind;
use crate::properties::{Card, CardOrigin, EditRoute, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

impl ShellModel {
    /// The find pane's state.
    pub fn find_state(&self) -> &FindState {
        &self.find
    }

    /// Whether the find pane is in the layout of the project view.
    pub fn find_shown(&self) -> bool {
        self.has_project()
            && self.wizard().is_none()
            && self.first_run().is_none()
            && self.layout().panes().contains(&PaneKind::Find)
    }

    /// Drops the results of another project session (the end of every dispatch).
    pub(super) fn sync_find(&mut self) {
        let epoch = self.project_epoch();
        if self.find.ran.as_ref().is_some_and(|r| Some(r.epoch) != epoch) {
            self.find.ran = None;
            self.find.row = None;
            self.find.note = None;
        }
    }

    /// The query the pane holds now: the replacement only in find-and-replace mode, the selection's records with "in
    /// selection".
    fn find_query(&self) -> FindQuery {
        FindQuery {
            text: self.find.text.clone(),
            regex: self.find.regex,
            match_case: self.find.match_case,
            whole_word: self.find.whole_word,
            fields: self.find.fields.clone(),
            folios: self.find.folios.clone(),
            within: if self.find.in_selection {
                self.tools.selection.items.iter().map(|e| e.0).collect()
            } else {
                Vec::new()
            },
            replace: self.find.replace_mode.then(|| self.find.replacement.clone()),
        }
    }

    /// One `Session::find` call, counted; the results replace the pane's and the cursor goes to the first hit.
    fn run_find(&mut self, query: FindQuery) -> Option<Arc<FindResults>> {
        let (Some(epoch), Some(rev), Some(project)) = (self.project_epoch(), self.revision(), &self.project)
        else {
            return None;
        };
        bump(&self.calls, |c| c.find += 1);
        let results = Arc::new(project.session.find(&query));
        self.find.row = (!results.hits.is_empty()).then_some(0);
        self.find.ran = Some(Ran {
            epoch,
            rev,
            query,
            results: results.clone(),
        });
        Some(results)
    }

    /// `find.open` (Ctrl+F) and `find.open_replace` (Ctrl+H): the pane below the page view, with the focus.
    pub(super) fn act_find_open(&mut self, replace: bool) -> Outcome {
        if !self.layout.panes().contains(&PaneKind::Find) {
            let layout = self.layout.with_find();
            self.replace_layout(layout);
        }
        self.find.replace_mode = replace;
        self.focus = Some(Focus::Find);
        Outcome::Done
    }

    /// `find.close`: the pane leaves the layout; its focus goes back to the canvas. A replace card made by the pane
    /// goes with it, since no pane would draw it and Ctrl+Enter in the property grid would apply it unseen.
    pub(super) fn act_find_close(&mut self) -> Outcome {
        let layout = self.layout.without(PaneKind::Find);
        self.replace_layout(layout);
        if self
            .props
            .card()
            .is_some_and(|c| matches!(c.origin, CardOrigin::Find { .. }))
        {
            self.props.card = None;
        }
        if self.focus == Some(Focus::Find) {
            self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        }
        Outcome::Done
    }

    /// `find.text` and `find.replacement`: the whole text of the field; no search (§9 item 4).
    pub(super) fn act_find_text(&mut self, p: &Map<String, Value>, replacement: bool) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed("error-missing-param", &["text"]),
        };
        if replacement {
            self.find.replacement = text;
        } else {
            self.find.text = text;
        }
        Outcome::Done
    }

    /// `find.option`: `option` (`regex`, `match_case`, `whole_word`, `in_selection`) and `on` (else it toggles).
    pub(super) fn act_find_option(&mut self, p: &Map<String, Value>) -> Outcome {
        let option = match string_param(p, "option") {
            Err(e) => return e,
            Ok(Some(o)) => o.to_owned(),
            Ok(None) => return failed("error-missing-param", &["option"]),
        };
        let flag = match option.as_str() {
            "regex" => &mut self.find.regex,
            "match_case" => &mut self.find.match_case,
            "whole_word" => &mut self.find.whole_word,
            "in_selection" => &mut self.find.in_selection,
            other => return failed("error-find-option", &[other]),
        };
        *flag = p.get("on").and_then(Value::as_bool).unwrap_or(!*flag);
        Outcome::Done
    }

    /// `find.fields`: `fields`, a list of field words; an empty list checks every field.
    pub(super) fn act_find_fields(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Value::Array(words)) = p.get("fields") else {
            return failed("error-missing-param", &["fields"]);
        };
        let mut fields = Vec::new();
        for w in words {
            let Some(word) = w.as_str() else {
                return failed("error-bad-param", &["fields"]);
            };
            match field_of_word(word) {
                Some(f) => fields.push(f),
                None => return failed("error-find-field", &[word]),
            }
        }
        // In the pane's order, each once; none checked searches every field, so the list shows all five.
        self.find.fields = crate::find::FIELDS
            .into_iter()
            .filter(|f| fields.is_empty() || fields.contains(f))
            .collect();
        Outcome::Done
    }

    /// `find.folios`: `folios`, a list of printed folio labels (the pane's filter splits its text at commas).
    pub(super) fn act_find_folios(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Value::Array(labels)) = p.get("folios") else {
            return failed("error-missing-param", &["folios"]);
        };
        let mut folios = Vec::new();
        for l in labels {
            match l.as_str() {
                Some(text) if !text.trim().is_empty() => folios.push(text.trim().to_owned()),
                Some(_) => {}
                None => return failed("error-bad-param", &["folios"]),
            }
        }
        self.find.folios = folios;
        Outcome::Done
    }

    /// `find.run` (Enter in the text): exactly one `Session::find` call. An error of the query shows in the pane.
    pub(super) fn act_find_run(&mut self) -> Outcome {
        if self.find.in_selection && self.tools.selection.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-find-no-selection",
            };
        }
        let query = self.find_query();
        self.find.note = None;
        self.run_find(query);
        Outcome::Done
    }

    /// `find.row`: `step` (±n, clamped) or `to` (1-based).
    pub(super) fn act_find_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let n = self.find.results().map_or(0, |r| r.hits.len());
        if n == 0 {
            return Outcome::Refused {
                reason_key: "reason-find-no-results",
            };
        }
        let at = if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return failed("error-bad-param", &["step"]);
            };
            let here = self.find.row().map_or(-1, |r| r as i64);
            let from = if here < 0 && step > 0 { -1 } else { here.max(0) };
            (from + step).clamp(0, n as i64 - 1) as usize
        } else {
            match p.get("to").and_then(Value::as_u64) {
                Some(to) if to >= 1 && (to as usize) <= n => to as usize - 1,
                Some(_) | None if p.contains_key("to") => return failed("error-bad-param", &["to"]),
                _ => return failed("error-missing-param", &["step"]),
            }
        };
        self.find.row = Some(at);
        Outcome::Done
    }

    /// `find.jump` (Enter on a result, a double click): the hit's record becomes the selection and its page is shown
    /// with the grid cursor on its cell (`nav.goto`); the pane stays open with the focus.
    pub(super) fn act_find_jump(&mut self) -> Outcome {
        let Some(hit) = self
            .find
            .row()
            .and_then(|r| self.find.results().and_then(|res| res.hits.get(r)).cloned())
        else {
            return Outcome::Refused {
                reason_key: "reason-find-no-results",
            };
        };
        let change = crate::tools::SelectionChange::Replace(vec![EntityRef(hit.record)]);
        if self.tools.selection.apply(&change) {
            self.tools.announcement = Some(strings::format(
                "announce-selected",
                &[&self.tools.selection.items.len().to_string()],
            ));
        }
        let focus = self.focus;
        let outcome = match hit.cell.as_deref().or(hit.folio.as_deref()) {
            Some(target) => {
                let mut p = Map::new();
                p.insert("text".to_owned(), Value::from(target));
                self.act_nav_goto(&p)
            }
            None => Outcome::Done,
        };
        if focus == Some(Focus::Find) {
            self.focus = focus;
        }
        outcome
    }

    /// `find.replace_all` (Ctrl+Enter in the pane, Edit ▸ Replace all): the results' `fields.set_many`, previewed,
    /// always through the card (§7.4). Stale results, or results of another query, are searched again first.
    pub(super) fn act_find_replace_all(&mut self) -> Outcome {
        if !self.find.replace_mode {
            return Outcome::Refused {
                reason_key: "reason-find-no-replace",
            };
        }
        let (Some(epoch), Some(rev)) = (self.project_epoch(), self.revision()) else {
            return Outcome::Done;
        };
        let query = self.find_query();
        let current = self
            .find
            .ran
            .as_ref()
            .is_some_and(|r| r.epoch == epoch && r.rev == rev && r.query == query);
        let results = if current {
            self.find.ran.as_ref().map(|r| r.results.clone())
        } else {
            if self.find.in_selection && self.tools.selection.is_empty() {
                return Outcome::Refused {
                    reason_key: "reason-find-no-selection",
                };
            }
            self.run_find(query.clone())
        };
        let Some(results) = results else {
            return Outcome::Done;
        };
        self.find.note = None;
        self.preview_replace(&results, query, false)
    }

    /// Previews the replace of `results` (made for `query`) and shows the card; `rebuilt` marks a stale card's
    /// second preview.
    fn preview_replace(&mut self, results: &FindResults, query: FindQuery, rebuilt: bool) -> Outcome {
        if let Some(e) = &results.error {
            return Outcome::Failed { message: e.clone() };
        }
        if results.truncated {
            return Outcome::Refused {
                reason_key: "reason-find-truncated",
            };
        }
        let Some(params) = results.replace_command() else {
            return Outcome::Refused {
                reason_key: "reason-find-nothing",
            };
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let name = "fields.set_many";
        match project.session.command_meta(name) {
            Some(meta) if meta.exposure == Exposure::Edit => {}
            Some(_) => {
                return Outcome::Refused {
                    reason_key: "reason-human-only",
                };
            }
            None => return failed("error-unknown-action", &[name]),
        }
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let hits = results.changes();
        let edits = params["edits"].as_array().cloned().unwrap_or_default();
        let targets: Vec<String> = edits
            .iter()
            .flat_map(|e| e["targets"].as_array().cloned().unwrap_or_default())
            .filter_map(|t| t.as_str().map(str::to_owned))
            .collect();
        let first = edits.first();
        let kind = first
            .and_then(|e| e["kind"].as_str())
            .unwrap_or("device")
            .to_owned();
        let path = first
            .and_then(|e| e["path"].as_str())
            .unwrap_or_default()
            .to_owned();
        let cmd = AnyCommand {
            name: name.to_owned(),
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
        let result = result.map_err(|e| self.refusal_for_find(&e, &params));
        let edit = PendingEdit {
            row: String::new(),
            kind: kind.clone(),
            route: EditRoute::FieldsSet {
                kind,
                path: path.clone(),
            },
            path,
            targets,
            leaf: None,
            base: None,
            name: name.to_owned(),
            params: params.clone(),
            typed: String::new(),
            label: String::new(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: name.to_owned(),
                result,
                derived,
            },
            targets: edit.targets.len(),
            changes: hits,
            reason: if rebuilt {
                "reason-rebuilt"
            } else {
                "reason-find-replace"
            },
            edit,
            origin: CardOrigin::Find { query, hits },
        });
        Outcome::Done
    }

    /// A command refusal of the replace as the pane shows it (§7.4, the refusal per field): a pointer below
    /// `/edits/<i>` names the entry's record by its subject in the results and the field by its path ("on -201U5-Q1
    /// (function_text): ..."); other IDs as folios or "a record".
    fn refusal_for_find(&self, e: &ew_api::CmdError, params: &Value) -> String {
        let subject = |uuid: &str| {
            self.find.results().and_then(|r| {
                r.hits
                    .iter()
                    .find(|h| h.record.to_string() == uuid)
                    .map(|h| h.subject.clone())
            })
        };
        let rows = self.page_rows();
        let name = |u: &str| {
            subject(u)
                .or_else(|| {
                    rows.iter()
                        .find(|r| r.id.to_string() == u)
                        .map(|r| r.folio.clone())
                })
                .unwrap_or_else(|| strings::text("props-a-record").to_owned())
        };
        let text = match e {
            ew_api::CmdError::InvalidParams { path, message, .. } => {
                let mut parts = path.trim_start_matches('/').split('/');
                match (parts.next(), parts.next().and_then(|i| i.parse::<usize>().ok())) {
                    (Some("edits"), Some(i)) => {
                        let entry = &params["edits"][i];
                        let k = match (parts.next(), parts.next().and_then(|k| k.parse::<usize>().ok())) {
                            (Some("targets"), Some(k)) => k,
                            _ => 0,
                        };
                        let target = entry["targets"][k].as_str().map(name).unwrap_or_default();
                        let field = entry["path"].as_str().unwrap_or_default();
                        strings::format("props-refused-on", &[&format!("{target} ({field})"), message])
                    }
                    _ => message.clone(),
                }
            }
            ew_api::CmdError::Rejected { reason, .. } => reason.clone(),
            ew_api::CmdError::Prevented { rule, finding, .. } => format!("{rule}: {finding}"),
            ew_api::CmdError::NotFound { .. } => strings::text("props-gone").to_owned(),
            other => other.to_string(),
        };
        super::properties::replace_uuids(&text, &name)
    }

    /// Apply on a replace card (§7.4): a fresh card commits one transaction; a stale one runs the search again and shows
    /// a fresh card.
    pub(super) fn apply_find_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let CardOrigin::Find { query, hits } = card.origin.clone() else {
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
                self.find.note = Some(strings::format("find-replaced", &[&hits.to_string()]));
            }
            return outcome;
        }
        // Stale: never the old parameters.
        let Some(results) = self.run_find(query.clone()) else {
            return Outcome::Done;
        };
        let now = results.changes();
        let outcome = self.preview_replace(&results, query, true);
        self.find.note = Some(updated_note(hits, now));
        outcome
    }

    /// The find pane's part of the Escape chain while it has the focus: its card goes, then the focus goes back to the
    /// canvas. `None` when the pane does not have the focus.
    pub(super) fn dismiss_find(&mut self) -> Option<Outcome> {
        if self.focus != Some(Focus::Find) {
            return None;
        }
        if self
            .props
            .card()
            .is_some_and(|c| matches!(c.origin, CardOrigin::Find { .. }))
        {
            self.props.card = None;
            return Some(Outcome::Done);
        }
        self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        Some(Outcome::Done)
    }
}
