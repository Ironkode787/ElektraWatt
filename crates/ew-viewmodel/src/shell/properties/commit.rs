// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Committing an edit of the Properties pane: a typed text or a value read, a layout row, a group's label, the pending edit and its card, a tag's proposal and rename, and the choices under a field.

use super::{GROUP_ROW, PRINTED_LABEL_ROW, TAG_ROW, form_choices, grid_choices, message_of, tag_edit_of};
use crate::key;
use crate::properties::{
    Card, CardOrigin, ChoiceKind, Choices, Decision, PendingEdit, Popup, PropRow, RowAction, RowMenuItem,
    RowValue, SheetSection, is_derived, kind_plural, member_path, policy,
};
use crate::properties::{GRID_ROW, STYLE_ROW};
use crate::shell::navigation::bump;
use crate::shell::tools::Kept;
use crate::shell::{Mode, Outcome, ShellModel, failed};
use crate::strings;
use crate::strings::Key;
use crate::tools::PreviewState;
use ew_api::output::table_edit::compose;
use ew_api::output::{InputError, ParsedInput};
use ew_api::{AnyCommand, Exposure};
use ew_dl::EntityRef;
use serde_json::{Map, Value, json};

impl ShellModel {
    /// Parses `text` for row `key` and commits it (a choice index picks an ambiguous reading).
    pub(super) fn commit_text(&mut self, key: &str, text: &str, choice: Option<usize>) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed(key!("error-no-row"), &[key]);
        };
        // A person of the project (Designer, Checker…) names the party of that role.
        if let Some(out) = self.commit_person(&row.key, text) {
            return out;
        }
        // Print wire numbers runs its own action, toast and all.
        if let Some(out) = self.commit_wire_numbers_row(&row.key, text) {
            return out;
        }
        // A terminal's Group row compiles through `Session::strip_edit`.
        if row.key == GROUP_ROW {
            return self.commit_group_label(&section, &row, text);
        }
        // A style typed by its name is that style.
        if row.key == STYLE_ROW
            && let Some(style) = self
                .text_style_rows()
                .into_iter()
                .find(|r| r.name.eq_ignore_ascii_case(text.trim()))
        {
            self.props.choices = None;
            return self.commit_value(key, Value::String(style.key), text.to_owned(), Vec::new());
        }
        // A Connection grid typed in its words is that grid.
        if row.key == GRID_ROW
            && let Some((_, value)) = grid_choices()
                .into_iter()
                .find(|(words, _)| words.eq_ignore_ascii_case(text.trim()))
        {
            self.props.choices = None;
            return self.commit_value(key, value, text.to_owned(), Vec::new());
        }
        // A form typed by its name in the window's language is that form.
        if matches!(row.ty.inner(), crate::properties::FieldType::Link { kind } if kind == "form")
            && let Some((id, _)) = self.project.as_ref().and_then(|p| {
                form_choices(&p.session)
                    .into_iter()
                    .find(|(_, name)| name.eq_ignore_ascii_case(text.trim()))
            })
        {
            self.props.choices = None;
            return self.commit_value(key, Value::String(id.to_string()), text.to_owned(), Vec::new());
        }
        // A word of an enumerated row is read back as its stored word.
        let typed = crate::properties::enum_value_of(&section.kind, &row, text);
        // A layout row compiles through `Session::layout_edit`.
        if matches!(row.route, crate::properties::EditRoute::Command { .. }) {
            return self.commit_layout_row(&section, &row, &typed);
        }
        // An existing device's tag proposes Join; the shown tag again builds no command.
        if row.key == TAG_ROW {
            if let Some(outcome) = self.propose_join(&section, key, text) {
                return outcome;
            }
            // Another device's tag that the symbol cannot join: the words say why it joins nothing (what that device has
            // free, what the symbol is); the tag is set as typed, a duplicate the checks then name.
            if let [device] = section.targets.as_slice()
                && let Some(m) = self
                    .project
                    .as_ref()
                    .and_then(|p| p.session.join_misfit(*device, text))
            {
                let free = if m.free.is_empty() {
                    strings::text(key!("props-join-misfit-none")).to_owned()
                } else {
                    m.free.join(", ")
                };
                self.notify(
                    strings::format(key!("props-join-misfit"), &[&m.tag, &free, &m.this]),
                    false,
                );
            }
            if matches!(&row.value, RowValue::One { text: shown, .. } if shown.trim() == text.trim()) {
                self.props.edit = None;
                self.props.error = None;
                self.props.tag_proposal = None;
                return Outcome::Done;
            }
        }
        if self
            .props
            .choices
            .as_ref()
            .is_some_and(|c| c.kind == ChoiceKind::Options)
        {
            self.props.choices = None;
        }
        let project = or_return!(crate::shell::gated(&self.project));
        // Frame of the lists reads a form as a list's Form row does, and refuses as it does.
        let (kind, path) = if row.key == crate::shell::list_look::LISTS_FRAME_ROW {
            ("report_block", "form")
        } else {
            (section.kind.as_str(), row.path.as_str())
        };
        let parsed = project.session.parse_field_input(kind, path, &typed);
        let parsed = match (parsed, choice) {
            (Ok(p), _) => p,
            (Err(InputError::Ambiguous { choices }), Some(i)) => match choices.get(i) {
                Some((_, v)) => ParsedInput {
                    value: v.clone(),
                    notes: Vec::new(),
                },
                None => return failed(key!("error-bad-param"), &["choice"]),
            },
            (Err(InputError::Ambiguous { choices }), None) => {
                self.props.choices = Some(Choices {
                    row: key.to_owned(),
                    items: choices,
                    details: Vec::new(),
                    kind: ChoiceKind::Ambiguous,
                    index: None,
                });
                return Outcome::Done;
            }
            (Err(InputError::Invalid(why)), _) => {
                // The reason in the window's language.
                let message = crate::shell::tools::rejected_words(&why.to_string(), Some(&why));
                self.error_under_row(key.to_owned(), message.clone());
                return Outcome::Failed { message };
            }
            (Err(InputError::UnknownPart(typed)), _) => {
                let message = strings::format(key!("error-no-part"), &[&typed]);
                self.error_under_row(key.to_owned(), message.clone());
                return Outcome::Failed { message };
            }
            (Err(InputError::NotOneOf { typed, options }), _) => {
                let message = crate::properties::not_one_of_text(&section.kind, &row.path, &typed, &options);
                self.error_under_row(key.to_owned(), message.clone());
                return Outcome::Failed { message };
            }
        };
        // A language object: the text replaces the source language's and the other languages stay; an empty
        // text removes the source language, and with it the last one the value.
        let value = match (self.mltext_object(&section, &row), parsed.value) {
            (Some(mut object), v @ (Value::String(_) | Value::Null)) => {
                let lang = self.source_language(&object);
                match v {
                    Value::String(t) if !t.is_empty() => {
                        object.insert(lang, Value::String(t));
                    }
                    _ => {
                        object.remove(&lang);
                    }
                }
                if object.is_empty() {
                    Value::Null
                } else {
                    Value::Object(object)
                }
            }
            (_, v) => v,
        };
        let out = self.commit_value(key, value, text.to_owned(), parsed.notes);
        // A panel page's scale at which its layout runs over the frame says so, with Fit the layout to the sheet.
        if out == Outcome::Done
            && row.key == "page/scale"
            && let [page] = section.targets.as_slice()
            && let Some((now, fit)) = self.project_session().and_then(|s| s.layout_overruns(*page))
        {
            let said = strings::format(
                key!("toast-layout-overruns"),
                &[&now.to_string(), &fit.to_string()],
            );
            self.notify_offer(said, crate::notifications::NoteAction::FitSheet, vec![*page]);
        }
        out
    }

    /// Compiles the leaf `value` of row `key` into exactly one command and runs the commit path.
    pub(in crate::shell) fn commit_value(
        &mut self,
        key: &str,
        value: Value,
        typed: String,
        notes: Vec<String>,
    ) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed(key!("error-no-row"), &[key]);
        };
        // Frame of the lists sets the Form of every list.
        if let Some(out) = self.commit_lists_frame(key, &value) {
            return out;
        }
        // A page's Connection grid changed while it holds items asks first, then fits them: every page, the
        // one on show or not, whatever it holds; `fields.set` would refuse it.
        if row.key == GRID_ROW
            && self.mode == crate::shell::Mode::Gui
            && let Some(page) = section.targets.first().copied()
            && let Some(word) = value
                .get("num")
                .and_then(Value::as_i64)
                .zip(value.get("den").and_then(Value::as_i64))
                .and_then(|(n, d)| crate::shell::creation::page_grid_word(n, d))
        {
            self.props.edit = None;
            self.props.error = None;
            self.props.choices = None;
            if let Some(out) = self.grid_question(page, word) {
                return out;
            }
        }
        // A bus device's PLC row gives it to the PLC chosen, or to a new one.
        if matches!(&row.route, crate::properties::EditRoute::Command { command } if command == "plc.add_to_plc")
        {
            self.props.edit = None;
            self.props.error = None;
            let mut params = Map::new();
            if let Some(device) = section.targets.first() {
                params.insert("device".to_owned(), Value::String(device.to_string()));
            }
            // A choice is the PLC's ID; a name typed is the PLC of that name.
            if let Value::String(typed) = value {
                if typed.trim().is_empty() {
                    return Outcome::Done;
                }
                let plcs = self.plc_config_projects();
                let Some((config, _)) = plcs
                    .iter()
                    .find(|(id, name)| id.to_string() == typed || name.eq_ignore_ascii_case(typed.trim()))
                else {
                    let message = strings::format(key!("props-plc-unknown"), &[typed.trim()]);
                    self.error_under_row(key.to_owned(), message.clone());
                    return Outcome::Failed { message };
                };
                params.insert("config".to_owned(), Value::String(config.to_string()));
            }
            return self.act_plc_add_to_plc(&params);
        }
        // A layout row's choice or toggle is its text.
        if matches!(row.route, crate::properties::EditRoute::Command { .. }) {
            let text = match &value {
                Value::String(s) => s.clone(),
                Value::Bool(b) => b.to_string(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            return self.commit_layout_row(&section, &row, &text);
        }
        let Some(member) = member_path(&row.route) else {
            return Outcome::refused(key!("reason-read-only"));
        };
        // The member's value on every target as the sheet shows it (the base of the edit).
        let member_key = format!("{}/{member}", section.kind);
        let base = if member_key == row.key && !is_derived(&row) {
            match &row.value {
                RowValue::One { json, .. } => Some(json.clone()),
                _ => None,
            }
        } else {
            match self.props_find(&member_key).map(|(_, r)| r.value) {
                Some(RowValue::One { json, .. }) => Some(json),
                Some(RowValue::Varies { .. }) if row.path != member => {
                    let message = strings::text(key!("props-member-varies")).to_owned();
                    self.error_under_row(key.to_owned(), message.clone());
                    return Outcome::Failed { message };
                }
                _ => None,
            }
        };
        let Some(new) = compose(&member, &row.path, base.as_ref(), &value) else {
            let message = strings::text(key!("props-cannot-compose")).to_owned();
            self.error_under_row(key.to_owned(), message.clone());
            return Outcome::Failed { message };
        };
        // A value equal to the current one builds no command.
        let current = if row.path == member {
            match &row.value {
                RowValue::One { json, .. } if !is_derived(&row) => Some(json.clone()),
                RowValue::Absent if !is_derived(&row) => Some(Value::Null),
                _ => base.clone(),
            }
        } else {
            base.clone()
        };
        let unchanged = current.as_ref() == Some(&new) || (current.is_none() && new.is_null());
        // The Printed label row is the function text under another key, not an override.
        if unchanged && (!(is_derived(&row) && row.path == member) || row.key == PRINTED_LABEL_ROW) {
            self.props.edit = None;
            self.props.error = None;
            self.props.tag_proposal = None;
            return Outcome::Done;
        }
        let edit = self.pending_edit(&section, &row, &member, value, new, base, typed);
        self.props.notes = notes;
        self.run_edit(edit, false)
    }

    /// A layout row: one `Session::layout_edit` of the row's text, through the commit
    /// path of every edit (one preview, the policy, the commit); the text on show again builds no command, and a text
    /// that does not read stays under the row in words.
    pub(super) fn commit_layout_row(
        &mut self,
        section: &crate::properties::SheetSection,
        row: &crate::properties::PropRow,
        text: &str,
    ) -> Outcome {
        let shown = match &row.value {
            RowValue::One { text, .. } => Some(text.clone()),
            RowValue::Absent => Some(String::new()),
            RowValue::Varies { .. } => None,
        };
        // The shown text, or the row's value (a choice by ID), again builds no command.
        let same_value =
            matches!(&row.value, RowValue::One { json: Value::String(v), .. } if v == text.trim());
        if shown.as_deref().map(str::trim) == Some(text.trim()) || same_value {
            self.props.edit = None;
            self.props.error = None;
            return Outcome::Done;
        }
        let project = or_return!(crate::shell::gated(&self.project));
        let Some(target) = section.targets.first().copied() else {
            return failed(key!("error-no-row"), &[&row.key]);
        };
        // A usual medium chosen: the ports the row defines its medium at, as for a medium of the project.
        let preset = text.strip_prefix(crate::shell::creation::PRESET_PREFIX);
        let edit = ew_api::output::LayoutEdit {
            kind: section.kind.clone(),
            target,
            others: section.targets.iter().skip(1).copied().collect(),
            row: row.path.clone(),
            text: preset.map_or_else(|| text.to_owned(), |_| ew_api::output::Uuid::nil().to_string()),
        };
        let cmd = match project.session.layout_edit(&edit) {
            Ok(c) => c,
            Err(ew_api::output::LayoutEditError::Refused(reason)) => {
                let message = crate::shell::text_words(&reason);
                self.error_under_row(row.key.clone(), message.clone());
                return Outcome::Failed { message };
            }
        };
        // The usual medium is made and defined at those ports in one step (`fluid.medium.edit {set, at}`).
        let cmd = match preset {
            Some(word) if cmd.name == "fluid.medium.define" => {
                let Some(set) = crate::shell::creation::medium_preset_set(word) else {
                    return failed(key!("error-bad-param"), &["preset"]);
                };
                let p = &cmd.params;
                let at: Vec<Value> = std::iter::once(json!({"function": p["function"], "idx": p["idx"]}))
                    .chain(p["also"].as_array().cloned().unwrap_or_default())
                    .collect();
                ew_api::AnyCommand {
                    name: "fluid.medium.edit".into(),
                    params: json!({"set": set, "at": at}),
                }
            }
            _ => cmd,
        };
        // A valve's position typed that another valve holds: the window offers to swap the two.
        if row.path == "derived.position"
            && let Some(parent) = cmd.params["parent"]
                .as_str()
                .and_then(|p| p.parse::<ew_api::output::Uuid>().ok())
            && let Some(position) = cmd.params["children"][0]["position"].as_u64()
            && let Some(holder) = project
                .session
                .valve_at(parent, position)
                .filter(|h| *h != target)
        {
            let tag = |d| project.session.device_tag(d).unwrap_or_default();
            let text = strings::format(
                key!("toast-position-held"),
                &[&position.to_string(), &tag(parent), &tag(holder)],
            );
            self.props.edit = None;
            self.props.error = None;
            self.notify_offer(text, crate::notifications::NoteAction::Swap, vec![target, holder]);
            return Outcome::Done;
        }
        // A new layout view gets its legend page (doc 12 §5.4).
        let new_view = row.path.ends_with("surface") && matches!(row.value, RowValue::Absent);
        let pending = PendingEdit {
            row: row.key.clone(),
            kind: section.kind.clone(),
            targets: section.targets.iter().map(ToString::to_string).collect(),
            route: row.route.clone(),
            path: row.path.clone(),
            leaf: None,
            base: None,
            name: cmd.name,
            params: cmd.params,
            typed: text.to_owned(),
            label: crate::properties::row_label(section, row),
        };
        let out = self.run_edit(pending, false);
        if out == Outcome::Done && new_view {
            self.add_view_legend();
        }
        out
    }

    /// The Group row of terminals: one `StripEdit::GroupLabel` over the section's terminals, through
    /// the commit path of every edit (one preview, the policy, the commit or the card); the same text builds no command.
    pub(super) fn commit_group_label(
        &mut self,
        section: &crate::properties::SheetSection,
        row: &crate::properties::PropRow,
        text: &str,
    ) -> Outcome {
        let label = text.trim().to_owned();
        let current = match &row.value {
            RowValue::One { text, .. } => Some(text.clone()),
            RowValue::Absent => Some(String::new()),
            RowValue::Varies { .. } => None,
        };
        if current.as_deref() == Some(label.as_str()) {
            self.props.edit = None;
            self.props.error = None;
            return Outcome::Done;
        }
        let project = or_return!(crate::shell::gated(&self.project));
        let edit = ew_api::output::StripEdit::GroupLabel {
            terminals: section.targets.clone(),
            label: label.clone(),
        };
        let cmd = match project.session.strip_edit(&edit) {
            Ok(c) => c,
            Err(ew_api::output::StripEditError::Refused(reason_key)) => {
                return Outcome::Refused {
                    reason: Key::run_time(reason_key).into(),
                };
            }
            Err(e) => {
                let message = message_of(&Err(e));
                self.error_under_row(row.key.clone(), message.clone());
                return Outcome::Failed { message };
            }
        };
        let pending = PendingEdit {
            row: row.key.clone(),
            kind: section.kind.clone(),
            targets: section.targets.iter().map(ToString::to_string).collect(),
            route: row.route.clone(),
            path: row.path.clone(),
            leaf: None,
            base: None,
            name: cmd.name,
            params: cmd.params,
            typed: text.to_owned(),
            label: crate::properties::row_label(section, row),
        };
        self.run_edit(pending, false)
    }

    /// The command of an edit.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn pending_edit(
        &self,
        section: &SheetSection,
        row: &PropRow,
        member: &str,
        leaf: Value,
        new: Value,
        base: Option<Value>,
        typed: String,
    ) -> PendingEdit {
        let targets: Vec<String> = section.targets.iter().map(ToString::to_string).collect();
        let (name, params) = match &row.route {
            crate::properties::EditRoute::SettingsMember { member: m } => (
                "project.settings.set".to_owned(),
                json!({ "member": m, "value": new }),
            ),
            crate::properties::EditRoute::SupplyMember { path } => (
                "project.settings.set".to_owned(),
                json!({ "member": format!("supply.{path}"), "value": new }),
            ),
            crate::properties::EditRoute::FieldsSet { kind, path } => {
                let mut params = json!({ "kind": kind, "targets": targets, "path": path, "value": new });
                // A part a library holds and the project does not (a tube part, a part use's part): embedded with the
                // edit.
                let copies = new
                    .get("copy")
                    .or_else(|| leaf.get("copy"))
                    .and_then(Value::as_str)
                    .and_then(|c| c.parse().ok())
                    .zip(self.project.as_ref())
                    .map(|(c, p)| p.session.module_part_copies(c))
                    .unwrap_or_default();
                if !copies.is_empty() {
                    params["copies"] = Value::Array(copies);
                }
                ("fields.set".to_owned(), params)
            }
            crate::properties::EditRoute::Command { .. } | crate::properties::EditRoute::ReadOnly(_) => {
                (String::new(), Value::Null)
            }
        };
        // A Parts row that chooses a library's part the project does not hold assigns it with
        // `part.assign`, which embeds the part with its closure in the same transaction.
        let (name, params) = match self
            .project
            .as_ref()
            .filter(|_| row.ty.is_part_uses())
            .and_then(|p| {
                p.session
                    .part_uses_assign(&section.targets, &new)
                    .and_then(Result::ok)
            }) {
            Some(cmd) => (cmd.name, cmd.params),
            None => (name, params),
        };
        // A module's start typed as the profile's text or a keyword goes through
        // `plc.set_start`, which reads it with the module's profile.
        let start_dir = match row.path.as_str() {
            "plc_module.start.input" => Some("input"),
            "plc_module.start.output" => Some("output"),
            _ => None,
        };
        let (name, params) = match (section.kind.as_str(), start_dir, &leaf) {
            ("device", Some(dir), Value::String(spec)) => (
                "plc.set_start".to_owned(),
                json!({ "modules": targets, "dir": dir, "spec": spec }),
            ),
            _ => (name, params),
        };
        PendingEdit {
            row: row.key.clone(),
            kind: section.kind.clone(),
            targets,
            route: row.route.clone(),
            path: row.path.clone(),
            leaf: (row.path != member).then_some(leaf),
            base,
            name,
            params,
            typed,
            label: crate::properties::row_label(section, row),
        }
    }

    /// The commit path of an edit: the exposure guard, one preview, the policy, then the commit or the card.
    /// `card` forces the card (a rebuilt stale card).
    pub(super) fn run_edit(&mut self, edit: PendingEdit, card: bool) -> Outcome {
        self.props.tag_proposal = None;
        let row = edit.row.clone();
        let project = or_return!(crate::shell::gated(&self.project));
        let Some(meta) = project.session.command_meta(&edit.name) else {
            return failed(key!("error-unknown-action"), &[&edit.name]);
        };
        if meta.exposure != Exposure::Edit {
            return Outcome::refused(key!("reason-human-only"));
        }
        if !self.is_writable() {
            return Outcome::refused(key!("reason-read-only"));
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: edit.name.clone(),
            params: edit.params.clone(),
        };
        let submit = self.tool_submit();
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
        let changes = result.as_ref().map_or(0, |p| p.diff().len());
        // The edited row or its settable member is an outgoing identifier (`plc_module.start.input` is below
        // `plc_module.start`).
        let member_key = member_path(&edit.route).map(|m| format!("{}/{m}", edit.kind));
        let identifier = self.props_find(&row).is_some_and(|(_, r)| r.identifier)
            || member_key
                .and_then(|k| self.props_find(&k))
                .is_some_and(|(_, r)| r.identifier);
        let targets = edit.targets.len().max(1);
        let decision = policy(targets, identifier, derived.tags.len(), meta.cost, result.is_ok());
        let section = self.props_find(&row).map(|(s, _)| s);
        match (decision, result) {
            (Decision::Error, Err(e)) => {
                let mut message = self.refusal(&e, section.as_ref());
                let mut shown = message.clone();
                if let Some(proposal) = self.tag_proposal_for(&edit) {
                    shown = strings::format(key!("props-tag-proposal"), &[&message, &proposal.tag]);
                    // The window offers the "Use …" button; a script or blind transcript is told the action instead.
                    message = if self.mode == Mode::Headless {
                        strings::format(key!("props-tag-proposal-script"), &[&message, &proposal.tag])
                    } else {
                        shown.clone()
                    };
                    self.props.tag_proposal = Some(proposal);
                }
                self.error_under_row(row, shown);
                self.props.card = None;
                Outcome::Failed { message }
            }
            (Decision::Direct, Ok(preview)) if !card => {
                self.props.card = None;
                let state = PreviewState {
                    base_rev: rev,
                    command: edit.params.clone(),
                    name: edit.name.clone(),
                    result: Ok(preview),
                    derived,
                };
                self.commit_edit(state, &edit)
            }
            (decision, result) => {
                let reason = match decision {
                    Decision::Card(r) => r,
                    _ => key!("reason-rebuilt"),
                };
                self.props.card = Some(Card {
                    preview: PreviewState {
                        base_rev: rev,
                        command: edit.params.clone(),
                        name: edit.name.clone(),
                        result: result.map_err(|e| self.refusal(&e, section.as_ref())),
                        derived,
                    },
                    targets: edit.targets.len(),
                    changes,
                    reason,
                    edit,
                    origin: CardOrigin::Properties,
                });
                self.props.edit = None;
                self.props.error = None;
                self.props.tag_proposal = None;
                Outcome::Done
            }
        }
    }

    /// Commits a kept, fresh preview of `edit` ([`ShellModel::commit`]); a commit with several targets shows the
    /// toast.
    pub(super) fn commit_edit(&mut self, state: PreviewState, edit: &PendingEdit) -> Outcome {
        let section = self.props_find(&edit.row).map(|(s, _)| s);
        let outcome = self.commit(
            edit.name.clone(),
            edit.params.clone(),
            Kept::Card(state, section.as_ref()),
        );
        match &outcome {
            Outcome::Done => {
                self.props.edit = None;
                self.props.error = None;
                self.props.tag_proposal = None;
                self.props.choices = None;
                self.props.popup = None;
                if edit.targets.len() > 1 {
                    self.notify(
                        strings::format(
                            key!("toast-set"),
                            &[
                                // Sentence case, as the table's toast.
                                &strings::in_sentence(&edit.label),
                                &edit.targets.len().to_string(),
                                &kind_plural(&edit.kind, edit.targets.len()),
                            ],
                        ),
                        true,
                    );
                }
            }
            Outcome::Failed { message } => {
                self.error_under_row(edit.row.clone(), message.clone());
            }
            Outcome::Refused { .. } => {}
        }
        outcome
    }

    /// The proposal of a refused single-device tag-letter edit (FR-NUM-029: refused with the next free counter
    /// proposed, never replaced silently): the letter with the counter cleared, which online numbering (FR-NUM-007)
    /// fills with the next free one in the same transaction. The proposal is structural, never read from the
    /// refusal's text: its preview must succeed and change the device's printed tag; the tag comes from that preview's
    /// derived tag change and the counter from the device's `tag.counter` in the preview's after-state. `None` for several targets, another field, a proposal whose preview
    /// fails, or one that changes no tag (a refusal the cleared counter does not cure).
    pub(super) fn tag_proposal_for(&mut self, edit: &PendingEdit) -> Option<crate::properties::TagProposal> {
        if edit.kind != "device" || edit.path != "tag.letter" || edit.targets.len() != 1 {
            return None;
        }
        let base_rev = self.revision()?;
        let letter = match edit.params.get("path").and_then(Value::as_str) {
            Some("tag.letter") => edit.params.get("value")?.clone(),
            Some("tag") => edit.params.get("value")?.get("letter")?.clone(),
            _ => return None,
        };
        let target = &edit.targets[0];
        let params = json!({"edits": [
            {"kind": "device", "targets": [target], "path": "tag.counter", "value": null},
            {"kind": "device", "targets": [target], "path": "tag.letter", "value": letter},
        ]});
        let cmd = AnyCommand {
            name: "fields.set_many".to_owned(),
            params: params.clone(),
        };
        let submit = self.tool_submit();
        let project = self.project.as_mut()?;
        let preview = project.session.preview_with(&cmd, submit).ok()?;
        let change = project
            .session
            .derived_diff(&preview)
            .tags
            .into_iter()
            .find(|t| t.key.to_string() == *target)
            .filter(|t| t.after != t.before)?;
        // The counter online numbering gave, read from the device in the preview's after-state: never from the printed
        // tag's trailing digits, which also hold the page in the strip format `{letter}{page}{counter}` (`-X1031`) and a
        // parent's digits in a child tag.
        let after = project.session.query_over(preview.candidate());
        let counter = after.devices().get(change.key)?.tag.counter?.to_string();
        Some(crate::properties::TagProposal {
            row: edit.row.clone(),
            tag: change.after,
            counter,
            params,
            base_rev,
            join: false,
        })
    }

    /// `text` typed into one device's Tag field names another device the device's one
    /// representation can join (`Session::join_proposal`): the rename is refused and the error line proposes the join,
    /// accepted by `props.accept_tag_proposal` ("Join -K1"). `None` otherwise (the field's other behaviour).
    pub(super) fn propose_join(&mut self, section: &SheetSection, key: &str, text: &str) -> Option<Outcome> {
        let [device] = section.targets.as_slice() else {
            return None;
        };
        let base_rev = self.revision()?;
        let proposal = self.project.as_ref()?.session.join_proposal(*device, text)?;
        // A free function of the device the symbol becomes is named ("-201Q1 has an undrawn solenoid 14: make this symbol
        // that?"); else the symbol joins as an auxiliary contact.
        let shown = match &proposal.slot {
            Some(slot) => strings::format(key!("props-join-slot-proposal"), &[&proposal.tag, slot]),
            None => strings::format(key!("props-join-proposal"), &[&proposal.tag]),
        };
        // The window offers the "Join …" button; a script or blind transcript is told the action instead.
        let message = if self.mode == Mode::Headless {
            strings::format(key!("props-join-proposal-script"), &[&shown])
        } else {
            shown.clone()
        };
        self.props.choices = None;
        self.error_under_row(key.to_owned(), shown);
        self.props.tag_proposal = Some(crate::properties::TagProposal {
            row: key.to_owned(),
            tag: proposal.tag,
            counter: String::new(),
            params: json!({"rep": proposal.rep.to_string(), "device": proposal.device.to_string()}),
            base_rev,
            join: true,
        });
        Some(Outcome::Failed { message })
    }

    /// `props.accept_tag_proposal` (the "Use -102F2" button under a refused tag-letter edit): commits the proposal
    /// in one undo step; the tag stays unfrozen and the toast names the new tag and its counter.
    pub(crate) fn act_props_accept_tag_proposal(&mut self) -> Outcome {
        let Some(proposal) = self.props.tag_proposal.take() else {
            return Outcome::refused(key!("reason-no-tag-proposal"));
        };
        // A proposal is accepted only while its refusal is on show and the project has not changed since (a stale
        // proposal would set another device's or another state's tag).
        let on_show = self.props.error.as_ref().is_some_and(|(r, _)| *r == proposal.row);
        if !on_show || self.revision() != Some(proposal.base_rev) {
            return Outcome::refused(key!("reason-no-tag-proposal"));
        }
        let section = self.props_find(&proposal.row).map(|(s, _)| s);
        let name = if proposal.join {
            "rep.join_device"
        } else {
            "fields.set_many"
        };
        let (outcome, notices) = self.run_logged_or_commit(name, &proposal.params, section.as_ref());
        if outcome == Outcome::Done {
            self.props.edit = None;
            self.props.error = None;
            self.props.tag_proposal = None;
            let toast = if proposal.join {
                let rep = proposal.params["rep"].as_str().and_then(|r| r.parse().ok());
                rep.and_then(|r| self.joined_toast(r, &notices))
            } else {
                Some(strings::format(
                    key!("toast-tag-proposal"),
                    &[&proposal.tag, &proposal.counter],
                ))
            };
            if let Some(toast) = toast {
                self.notify(toast, true);
            }
            // The proposal of a device row's F2 rename closes its inline field.
            if self.rename.as_ref().is_some_and(|r| r.tag) {
                self.rename = None;
            }
        }
        outcome
    }

    /// Shows the sheet of `device` in the grid (the pane opened when it is closed, the device the selection), so
    /// that a navigator's F2 on a device row edits the tag through the Tag field's own path. The tag on show, or `None`
    /// when the sheet has no editable Tag row.
    pub(in crate::shell) fn tag_rename_sheet(&mut self, device: ew_api::output::Uuid) -> Option<String> {
        if !self
            .panes
            .layout
            .panes()
            .contains(&crate::layout::PaneKind::Properties)
        {
            let _ = self.act_props_show(&Map::new());
        }
        let items = vec![EntityRef(device)];
        if self.tools.selection.items != items {
            let _ = self
                .tools
                .selection
                .apply(&crate::tools::SelectionChange::Replace(items));
        }
        self.sync_properties();
        let (section, row) = self.props_find(TAG_ROW)?;
        if section.targets != [device] || self.props_refusal(&section, &row, RowAction::Edit).is_some() {
            return None;
        }
        match row.value {
            RowValue::One { text, .. } => Some(text),
            _ => Some(String::new()),
        }
    }

    /// Commits `text` typed for the tag of `device` (a navigator's F2 on a device row) through the grid's own
    /// rows: a tag another device prints and the device can join goes to the Tag row (its "Join -K1" proposal); a text
    /// that changes only the letter or only the counter of the tag on show goes to the Letter or the Counter row
    /// (FR-NUM-029: a counter collision on the page is refused, a letter edit with the next free counter proposed);
    /// any other text goes to the Tag row (the typed tag frozen). The refusal and the proposal stay in the grid's state
    /// ([`ShellModel::nav_rename_error`] shows them under the inline field).
    pub(in crate::shell) fn tag_rename_commit(
        &mut self,
        device: ew_api::output::Uuid,
        text: &str,
    ) -> Outcome {
        let Some(shown) = self.tag_rename_sheet(device) else {
            return Outcome::refused(key!("reason-nav-not-renamable"));
        };
        let row_text = |s: &Self, key: &str| match s.props_find(key).map(|(_, r)| r.value) {
            Some(RowValue::One { text, .. }) => text,
            _ => String::new(),
        };
        let letter = row_text(self, "device/tag.letter");
        let counter = row_text(self, "device/tag.counter");
        let joins = self
            .project
            .as_ref()
            .is_some_and(|p| p.session.join_proposal(device, text).is_some());
        let (row, value) = match tag_edit_of(&shown, &letter, &counter, text) {
            Some((row, value)) if !joins => (row, value),
            _ => (TAG_ROW, text.to_owned()),
        };
        let p = json!({"row": row, "value": value});
        match p {
            Value::Object(m) => self.act_props_set(&m),
            _ => Outcome::Done,
        }
    }

    /// The refusal under a device row's F2 field (the grid's error line of the tag's rows) and its proposal.
    pub fn nav_rename_error(&self) -> Option<(&str, Option<&crate::properties::TagProposal>)> {
        self.rename.as_ref().filter(|r| r.tag)?;
        let (row, message) = self.props.error()?;
        if !matches!(row, TAG_ROW | "device/tag.letter" | "device/tag.counter") {
            return None;
        }
        let proposal = self.props.tag_proposal().filter(|p| p.row == row);
        Some((message, proposal))
    }

    /// The toast after `rep` joined a device: "Joined -K1 (NO contact 13-14) at 102-C4", then what the removed
    /// source device carried (`notices`); `None` for a representation that draws no function of a device.
    pub(in crate::shell) fn joined_toast(
        &self,
        rep: ew_api::output::Uuid,
        notices: &[String],
    ) -> Option<String> {
        let j = self.project.as_ref()?.session.joined_text(rep)?;
        // The function's kind in the window's language.
        let kind = strings::render(&j.kind);
        let text = strings::format(key!("toast-joined"), &[&j.tag, &kind, &j.place]);
        Some(if notices.is_empty() {
            text
        } else {
            strings::format(
                key!("toast-joined-dropped"),
                &[strings::without_stop(&text), &notices.join("; ")],
            )
        })
    }

    /// One command through the grid's commit path: a fresh preview, then [`ShellModel::commit`]; with the
    /// notices of the command's outcome (what it dropped instead of refusing), read on its preview.
    pub(in crate::shell) fn run_logged_or_commit(
        &mut self,
        name: &str,
        params: &Value,
        section: Option<&SheetSection>,
    ) -> (Outcome, Vec<String>) {
        let Some(rev) = self.revision() else {
            return (Outcome::Done, Vec::new());
        };
        if !self.is_writable() {
            return (Outcome::refused(key!("reason-read-only")), Vec::new());
        }
        let cmd = AnyCommand {
            name: name.to_owned(),
            params: params.clone(),
        };
        let submit = self.tool_submit();
        let Some(project) = &mut self.project else {
            return (Outcome::Done, Vec::new());
        };
        let result = project.session.preview_with(&cmd, submit);
        let derived = match &result {
            Ok(p) => project.session.derived_diff(p),
            Err(_) => Default::default(),
        };
        let state = PreviewState {
            base_rev: rev,
            command: params.clone(),
            name: name.to_owned(),
            result: result.map_err(|e| self.refusal(&e, section)),
            derived,
        };
        let notices = match &state.result {
            Ok(p) => crate::shell::tools::notices_words(&p.outcome().notices),
            Err(message) => {
                return (
                    Outcome::Failed {
                        message: message.clone(),
                    },
                    Vec::new(),
                );
            }
        };
        (
            self.commit(name.to_owned(), params.clone(), Kept::Card(state, section)),
            notices,
        )
    }

    /// `props.choose`: `index` of the choices under a field (an ambiguous reading, the multi-language switch, a choice
    /// editor's options) or of the row's actions popup commits it; `step` (±1) moves the choices' highlight (Up,
    /// Down); with neither the highlighted choice is committed, else the buffer (Enter in a choice editor).
    pub(crate) fn act_props_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return failed(key!("error-bad-param"), &["step"]);
            };
            let Some(c) = &mut self.props.choices else {
                return Outcome::refused(key!("reason-no-choices"));
            };
            if c.items.is_empty() {
                return Outcome::Done;
            }
            let last = c.items.len() as i64 - 1;
            let at = match c.index {
                None if step > 0 => 0,
                None => last,
                Some(i) => (i as i64 + step).clamp(0, last),
            };
            c.index = Some(at as usize);
            return Outcome::Done;
        }
        let index = match p.get("index") {
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => i,
                None => return failed(key!("error-bad-param"), &["index"]),
            },
            None => match self.props.choices.as_ref().and_then(|c| c.index) {
                Some(i) => i,
                None if self.props.edit.is_some() => return self.act_props_commit(&Map::new()),
                None => return failed(key!("error-missing-param"), &["index"]),
            },
        };
        if let Some(Popup::Actions { row, .. }) = self.props.popup.clone() {
            if self.props_find(&row).is_none() {
                return failed(key!("error-no-row"), &[&row]);
            }
            let Some(item) = self.row_menu(&row).get(index).copied() else {
                return failed(key!("error-bad-param"), &["index"]);
            };
            self.props.popup = None;
            return match item {
                RowMenuItem::Action(action) => self.row_action(&row, action),
                RowMenuItem::MakeVariable => {
                    let mut params = Map::new();
                    params.insert("row".to_owned(), Value::String(row));
                    self.act_props_make_variable(&params)
                }
                // The Parts row's Choose part…, Add part… and Edit part….
                RowMenuItem::ChoosePart { first_use } => {
                    let mut params = Map::new();
                    if first_use {
                        params.insert("use".to_owned(), json!(0));
                    }
                    self.act_part_choose(&params)
                }
                RowMenuItem::AddPart => {
                    let mut params = Map::new();
                    params.insert("add".to_owned(), Value::Bool(true));
                    self.act_part_choose(&params)
                }
                RowMenuItem::EditPart => self.act_part_edit(&Map::new()),
                RowMenuItem::Placeholder => self.act_part_placeholder(&Map::new()),
                RowMenuItem::GoToSource => self.props_go_to_source(&row),
                RowMenuItem::DefinePotential => self.act_potential_draw(&Map::new()),
                RowMenuItem::CpUnused { on } => {
                    let point = match self.props_find(&row).map(|(_, r)| r.value) {
                        Some(RowValue::One { json, .. }) => json,
                        _ => return failed(key!("error-no-row"), &[&row]),
                    };
                    let mut params = Map::new();
                    params.insert("function".to_owned(), point["function"].clone());
                    params.insert("idx".to_owned(), point["idx"].clone());
                    params.insert("on".to_owned(), Value::Bool(on));
                    self.act_cp_unused(&params)
                }
                RowMenuItem::FunctionOverPath => self.dispatch("text.function_over_path", &Value::Null),
            };
        }
        let Some(choices) = self.props.choices.take() else {
            return Outcome::refused(key!("reason-no-choices"));
        };
        let Some((_, value)) = choices.items.get(index).cloned() else {
            self.props.choices = Some(choices);
            return failed(key!("error-bad-param"), &["index"]);
        };
        // New medium… opens its form; the medium it makes is set on the row's tubes.
        if value.is_null()
            && self.props_find(&choices.row).is_some_and(|(_, r)| {
                matches!(&r.route, crate::properties::EditRoute::Command { command } if command == "fluid.medium.define")
            })
        {
            self.props.edit = None;
            let out = self.act_fluid_medium_new(&Map::new());
            self.props.medium_for = Some(choices.row.clone());
            return out;
        }
        let typed = self
            .props
            .edit
            .as_ref()
            .map_or_else(String::new, |e| e.buffer.clone());
        match choices.kind {
            ChoiceKind::Ambiguous | ChoiceKind::MlTextSwitch | ChoiceKind::Options => {
                self.commit_value(&choices.row, value, typed, Vec::new())
            }
        }
    }
}
