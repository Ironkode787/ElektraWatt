// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Editing a row of the Properties pane: the editor opened with its value's text, its options, what is typed, Escape and Enter.

use super::{SURFACE_ROW, form_choices, grid_choices};
use crate::key;
use crate::properties::{
    ChoiceKind, Choices, EditBuffer, PropRow, RowAction, RowMenuItem, RowValue, SheetSection, find_row,
    is_derived, visible_rows,
};
use crate::properties::{GRID_ROW, STYLE_ROW};
use crate::shell::navigation::{Focus, bump};
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use serde_json::{Map, Value, json};

impl ShellModel {
    /// `props.edit`: opens the editor of `row` (else the focused row) with its value's text.
    pub(crate) fn act_props_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        // With `record` (its ID: a finding's Type the length…) the sheet shows that record first.
        if let Some(record) = or_return!(string_param(p, "record")) {
            let out = self.act_select_set(&Map::from_iter([("ids".to_owned(), json!([record]))]));
            if out != Outcome::Done {
                return out;
            }
            self.sync_properties();
            self.focus = Some(Focus::Properties);
        }
        let key = or_return!(self.props_row_param(p));
        // The Potential row of a potential no definition names: Enter, F2 or a click is Define potential…, as the
        // wire's menu offers it.
        if self.row_menu(&key).contains(&RowMenuItem::DefinePotential) {
            return self.act_potential_draw(&Map::new());
        }
        // A derived row edits its authored partner: Enter or F2 is its Override.
        let overrides = self.props_find(&key).is_some_and(|(_, r)| {
            !r.actions.contains(&RowAction::Edit) && r.actions.contains(&RowAction::Override)
        });
        if overrides {
            return self.row_action(&key, RowAction::Override);
        }
        self.begin_edit(&key, RowAction::Edit, None)
    }

    /// Opens the editor of `key` for `action` (Edit or Override) with `buffer`, else the value's text.
    pub(in crate::shell) fn begin_edit(
        &mut self,
        key: &str,
        action: RowAction,
        buffer: Option<String>,
    ) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed(key!("error-no-row"), &[key]);
        };
        if let Some(out) = self.props_refusal(&section, &row, action) {
            // A wire's value its cable core gives says so under the row, where the person tried (click,
            // double-click and typing did nothing); its button goes to the cable.
            if let Some(tag) = self.cable_row(key) {
                let message = strings::format(key!("props-set-by-cable"), &[&tag]);
                self.error_under_row(key.to_owned(), message.clone());
                return Outcome::Failed { message };
            }
            // A generated page's row says under it why, with the button to where it is changed.
            if let (Some(_), Outcome::Failed { message }) = (self.row_offer(key), &out) {
                self.error_under_row(key.to_owned(), message.clone());
            }
            // Tips 2 and 3.
            self.tip_for_refused_row(&section, &row, &out);
            return out;
        }
        // A bus line's Cable and Plugs rows open its Cable form (doc 10 §2.7).
        if matches!(&row.route, crate::properties::EditRoute::Command { command } if command == "bus.cable") {
            return self.act_bus_cable(&Map::new());
        }
        let shown = buffer.is_none();
        let buffer = match buffer {
            Some(b) => b,
            None => match (&row.value, self.mltext_object(&section, &row)) {
                // A multi-language value edits its source language's text inside the object.
                (_, Some(object)) => self.source_text(&object),
                // The minimal part chooser edits the part numbers (`533356 × 1`), not the list's JSON.
                (RowValue::One { json, text }, None) if row.ty.is_part_uses() => {
                    if json.as_array().is_some_and(Vec::is_empty) {
                        String::new()
                    } else {
                        text.clone()
                    }
                }
                // The Connection grid is edited in its words ("4 mm"), as its choices show it.
                (RowValue::One { text, .. }, None) if row.key == GRID_ROW || row.key == STYLE_ROW => {
                    text.clone()
                }
                // A JSON value shown in words (a scale's `1:10`, a length's `12.5 m`) is edited in
                // those words, which its parser reads back.
                (RowValue::One { json, text }, None)
                    if matches!(row.ty.inner(), crate::properties::FieldType::Json)
                        && serde_json::from_str::<Value>(text).ok().as_ref() != Some(json) =>
                {
                    text.clone()
                }
                (RowValue::One { json, text }, None) => {
                    if row.ty.is_container() || matches!(row.ty.inner(), crate::properties::FieldType::Json) {
                        json.to_string()
                    } else {
                        text.clone()
                    }
                }
                // "Set all" (doc 16 §6.2): the first target's value of a list or group.
                (RowValue::Varies { .. }, None) if row.ty.is_container() => {
                    self.first_target_json(&section, &row)
                }
                _ => String::new(),
            },
        };
        self.props.row = Some(key.to_owned());
        self.props.edit = Some(EditBuffer {
            row: key.to_owned(),
            initial: shown.then(|| buffer.clone()),
            buffer,
        });
        self.props.error = None;
        self.props.tag_proposal = None;
        self.props.choices = None;
        self.props.popup = None;
        // A choice editor lists its options.
        if crate::properties::opens_choices(&row) {
            self.props.choices = self.options(&section, &row, "", key);
        }
        Outcome::Done
    }

    /// The options of a choice editor for `row` matching `filter` (`Session::field_choices`).
    pub(super) fn options(
        &self,
        section: &SheetSection,
        row: &PropRow,
        filter: &str,
        key: &str,
    ) -> Option<Choices> {
        let p = self.project.as_ref()?;
        bump(&self.calls, |c| c.field_choices += 1);
        // A layout row lists the choices its sheet built (the device's parts, the turns, the
        // project's surfaces), each its own text.
        let mut details = Vec::new();
        // A frame row's choices: Shows', the text styles, the alignments.
        if let Some(items) = self.frame_row_choices(&row.key, filter) {
            return Some(Choices {
                row: key.to_owned(),
                items,
                details,
                kind: ChoiceKind::Options,
                index: None,
            });
        }
        let items: Vec<(String, Value)> = match (&row.route, row.ty.inner()) {
            _ if row.key == GRID_ROW => {
                let f = filter.to_lowercase();
                grid_choices()
                    .into_iter()
                    .filter(|(text, _)| text.to_lowercase().contains(&f))
                    .collect()
            }
            // A text's style by its name; choosing one commits the style's key.
            _ if row.key == STYLE_ROW => {
                let f = filter.to_lowercase();
                self.text_style_rows()
                    .into_iter()
                    .filter(|r| r.name.to_lowercase().contains(&f))
                    .map(|r| (r.name, Value::String(r.key)))
                    .collect()
            }
            // An address profile by its name, its reference as the secondary line; choosing one commits the
            // reference.
            (crate::properties::EditRoute::Command { command }, _) if command == "plc.set_profile" => {
                let f = filter.to_lowercase();
                let (items, refs): (Vec<_>, Vec<_>) = p
                    .session
                    .profile_choices()
                    .into_iter()
                    .filter(|(r, name)| name.to_lowercase().contains(&f) || r.to_lowercase().contains(&f))
                    .map(|(r, name)| ((name, Value::String(r.clone())), r))
                    .unzip();
                details = refs;
                items
            }
            // A tube's medium by its name; choosing one commits the medium's ID. While the project has none, the usual
            // media of New medium… (a list with one entry, New medium…, offered no ready medium), each made and
            // set in one step. New medium… stands last.
            (crate::properties::EditRoute::Command { command }, _) if command == "fluid.medium.define" => {
                let f = filter.to_lowercase();
                let media = p.session.media();
                let usual = media
                    .is_empty()
                    .then(crate::shell::creation::medium_preset_choices);
                media
                    .into_iter()
                    .map(|(id, name)| (name, Value::String(id.to_string())))
                    .chain(usual.into_iter().flatten())
                    .filter(|(name, _)| name.to_lowercase().contains(&f))
                    .chain([(
                        strings::text(key!("action-fluid-medium-new")).to_owned(),
                        Value::Null,
                    )])
                    .collect()
            }
            // A bus device's PLC: the project's PLCs by name, then New PLC…; choosing one runs Add to
            // PLC.
            (crate::properties::EditRoute::Command { command }, _) if command == "plc.add_to_plc" => {
                let f = filter.to_lowercase();
                p.session
                    .plc_config_projects()
                    .into_iter()
                    .filter(|(_, name)| name.to_lowercase().contains(&f))
                    .map(|(id, name)| (name, Value::String(id.to_string())))
                    .chain([(strings::text(key!("props-plc-new")).to_owned(), Value::Null)])
                    .collect()
            }
            // The Surface row's choice is the surface, by its ID: two surfaces may read alike.
            (crate::properties::EditRoute::Command { .. }, _) if row.key == SURFACE_ROW => {
                let f = filter.to_lowercase();
                p.session
                    .surface_choices()
                    .into_iter()
                    .map(|(id, w)| (strings::render(&w), id))
                    .filter(|(w, _)| w.to_lowercase().contains(&f))
                    .map(|(w, id)| (w, Value::String(id.to_string())))
                    .collect()
            }
            (
                crate::properties::EditRoute::Command { .. },
                crate::properties::FieldType::Enum { options },
            ) => {
                let f = filter.to_lowercase();
                options
                    .iter()
                    .map(|o| {
                        (
                            crate::properties::command_choice_text(&row.path, o),
                            Value::String(o.clone()),
                        )
                    })
                    .filter(|(text, _)| text.to_lowercase().contains(&f))
                    .collect()
            }
            // A page's or a report block's form: the project's forms by name in the window's language.
            (_, crate::properties::FieldType::Link { kind }) if kind == "form" => {
                let f = filter.to_lowercase();
                form_choices(&p.session)
                    .into_iter()
                    .filter(|(_, name)| name.to_lowercase().contains(&f))
                    .map(|(id, name)| (name, Value::String(id.to_string())))
                    .collect()
            }
            // An enumerated row's choices are its words in the UI language, filtered by those words.
            (_, crate::properties::FieldType::Enum { options }) => {
                let f = filter.to_lowercase();
                options
                    .iter()
                    .map(|o| {
                        (
                            strings::field_choice_text(&section.kind, &row.path, o),
                            Value::String(o.clone()),
                        )
                    })
                    .filter(|(text, _)| text.to_lowercase().contains(&f))
                    .collect()
            }
            _ => p
                .session
                .field_choices(&section.kind, &row.path, filter)
                .into_iter()
                .map(|c| (c.text, c.value))
                .collect(),
        };
        Some(Choices {
            row: key.to_owned(),
            items,
            details,
            kind: ChoiceKind::Options,
            index: None,
        })
    }

    /// The stored language object of a multi-language row, if its value is one: the row's own value, or a derived
    /// row's authored partner's.
    pub(super) fn mltext_object(
        &self,
        section: &SheetSection,
        row: &PropRow,
    ) -> Option<serde_json::Map<String, Value>> {
        if !matches!(row.ty.inner(), crate::properties::FieldType::MlText) {
            return None;
        }
        let value = if is_derived(row) {
            self.props_find(&format!("{}/{}", section.kind, row.path))
                .map(|(_, r)| r.value)?
        } else {
            row.value.clone()
        };
        match value {
            RowValue::One {
                json: Value::Object(m),
                ..
            } => Some(m),
            _ => None,
        }
    }

    /// The project's source language (the sheet's first language), else the object's first.
    pub(super) fn source_language(&self, object: &serde_json::Map<String, Value>) -> String {
        self.props
            .sheet()
            .and_then(|s| s.languages.first().cloned())
            .or_else(|| object.keys().next().cloned())
            .unwrap_or_else(|| "en".to_owned())
    }

    /// The source language's text of a language object.
    pub(super) fn source_text(&self, object: &serde_json::Map<String, Value>) -> String {
        let lang = self.source_language(object);
        object
            .get(&lang)
            .or_else(|| object.values().next())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    }

    /// The JSON text of `row` on the section's first target: one sheet of that record (`props.edit` of a `‹varies›`
    /// list or group, "set all").
    pub(super) fn first_target_json(&self, section: &SheetSection, row: &PropRow) -> String {
        let (Some(first), Some(p)) = (section.targets.first(), &self.project) else {
            return String::new();
        };
        bump(&self.calls, |c| c.property_sheet += 1);
        let sheet = p.session.property_sheet(&[*first]);
        find_row(&sheet, &row.key)
            .and_then(|(_, r)| match &r.value {
                RowValue::One { json, .. } => Some(json.to_string()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// `props.input`: the whole buffer; on a focused editable row that is not editing it starts the edit.
    pub(crate) fn act_props_input(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["text"]),
        };
        if self.props.edit.is_none() {
            let Some(key) = self.properties_row() else {
                return Outcome::refused(key!("reason-no-row"));
            };
            let started = self.begin_edit(&key, RowAction::Edit, Some(String::new()));
            if started != Outcome::Done {
                return started;
            }
        }
        if let Some(e) = &mut self.props.edit {
            e.buffer.clone_from(&text);
        }
        self.props.error = None;
        self.props.tag_proposal = None;
        // A choice editor filters its options by the buffer; other choices go.
        let options = self
            .props
            .choices
            .as_ref()
            .is_some_and(|c| c.kind == ChoiceKind::Options);
        self.props.choices = None;
        if options
            && let Some(row_key) = self.props.edit.as_ref().map(|e| e.row.clone())
            && let Some((section, row)) = self.props_find(&row_key)
        {
            self.props.choices = self.options(&section, &row, &text, &row_key);
        }
        Outcome::Done
    }

    /// `props.cancel` (Escape in the field): the stored value comes back.
    pub(crate) fn act_props_cancel(&mut self) -> Outcome {
        self.props.edit = None;
        self.props.error = None;
        self.props.tag_proposal = None;
        self.props.choices = None;
        self.props.popup = None;
        Outcome::Done
    }

    /// `props.commit {step?}` (Enter, Tab, focus loss): the buffer is parsed and committed. With `step` (Tab 1,
    /// Shift+Tab −1) the editor then opens on the next (the previous) row typed in, so that a sheet's fields are
    /// filled one after the other (a text typed after Tab replaced the one before in the same field).
    pub(crate) fn act_props_commit(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(edit) = self.props.edit.clone() else {
            return Outcome::refused(key!("reason-not-editing"));
        };
        // The stored value's text committed unchanged: nothing to write (its display form need not parse back).
        let out = if edit.initial.as_deref() == Some(edit.buffer.as_str()) {
            self.act_props_cancel()
        } else {
            self.commit_text(&edit.row, &edit.buffer, None)
        };
        let step = p.get("step").and_then(Value::as_i64).unwrap_or(0).signum();
        // A refusal, a proposal or a choice keeps its row.
        if step == 0 || out != Outcome::Done || self.props.edit.is_some() || self.props.card().is_some() {
            return out;
        }
        match self.typed_row_after(&edit.row, step) {
            Some(next) => self.begin_edit(&next, RowAction::Edit, None),
            None => out,
        }
    }

    /// The row after `key` (`step` 1) or before it (−1) in the sheet's visible order whose value is typed in its
    /// field: one that can be edited now, not a checkbox and not a form's button. `None` at the sheet's end.
    pub(super) fn typed_row_after(&self, key: &str, step: i64) -> Option<String> {
        let sheet = self.props.sheet.clone()?;
        let collapsed = self.properties_collapsed();
        let filter = self.properties_filter().to_owned();
        let rows = visible_rows(
            &sheet,
            &collapsed,
            self.settings.properties_show_advanced,
            &filter,
        );
        let here = rows.iter().position(|r| r.row.key == key)?;
        let typed = |i: usize| {
            let row = rows[i].row;
            let section = sheet
                .sections
                .iter()
                .find(|s| s.rows.iter().any(|r| r.key == row.key))?;
            let form = matches!(&row.route, crate::properties::EditRoute::Command { command } if command == "bus.cable");
            (row.actions.contains(&RowAction::Edit)
                && !matches!(row.ty.inner(), crate::properties::FieldType::Bool)
                && !form
                && self.props_refusal(section, row, RowAction::Edit).is_none())
            .then(|| row.key.clone())
        };
        if step > 0 {
            (here + 1..rows.len()).find_map(typed)
        } else {
            (0..here).rev().find_map(typed)
        }
    }

    /// `props.set`: `row`, `value` (the text as typed), optional `choice` (an index for an ambiguous value).
    pub(crate) fn act_props_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(self.props_row_param(p));
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(Some(v)) => v.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["value"]),
        };
        let choice = match p.get("choice") {
            None => None,
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => Some(i),
                None => return failed(key!("error-bad-param"), &["choice"]),
            },
        };
        // A derived row sets its authored partner, as its Override does (Enter or F2 there, `props.edit`).
        let overrides = self.props_find(&key).is_some_and(|(_, r)| {
            !r.actions.contains(&RowAction::Edit) && r.actions.contains(&RowAction::Override)
        });
        let action = if overrides {
            RowAction::Override
        } else {
            RowAction::Edit
        };
        let started = self.begin_edit(&key, action, Some(value.clone()));
        if started != Outcome::Done {
            return started;
        }
        self.commit_text(&key, &value, choice)
    }
}
