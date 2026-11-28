// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fields of the project's own in the window (doc 09 §2.10): their rows under Fields of your own in Project
//! properties (each project field's value) and Page properties (each page field's value), New field of your own…,
//! and a row's Rename… and Remove.

use ew_api::AnyCommand;
use serde_json::{Map, Value, json};

use super::{Dialog, FormDialog, FormField, Mode, Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::properties::{EditRoute, FieldType, PropRow, PropertySheet, RowAction, RowValue};
use crate::strings;

/// The row path of a field of the project's own: `own.<key>`.
pub const OWN: &str = "own.";

/// The row of Project properties ▸ Fields of your own that makes a new field.
pub const OWN_NEW_ROW: &str = "project/own.new";

/// New field of your own…'s action.
const FIELD_NEW: &str = "field.new";

/// Rename…'s action.
const FIELD_RENAME: &str = "field.rename";

/// Whether `row` is a field of the project's own or the row that makes one.
pub fn is_own_row(row: &PropRow) -> bool {
    row.path.starts_with(OWN)
}

/// The key of the field of the project's own that row `key` shows (`page/own.order` → `order`); `None` for another row
/// and for the row that makes one.
pub fn own_key(key: &str) -> Option<&str> {
    let (_, rest) = key.split_once('/')?;
    rest.strip_prefix(OWN).filter(|_| key != OWN_NEW_ROW)
}

/// The text of a multi-language name in the window's language.
fn words(ml: ew_api::TextArg) -> String {
    strings::render(&ew_api::Text::bare(ml))
}

impl ShellModel {
    /// The project's sheet gets a row per field of the whole project with its value, and the row New field of your
    /// own…; a page's sheet a row per field of each page with the page's value (the value of several pages when it is
    /// the same, else that they differ). Both stand in the group Fields of your own.
    pub(super) fn own_fields_in_rows(&self, sheet: &mut PropertySheet) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        for section in &mut sheet.sections {
            let page = match section.kind.as_str() {
                "project" => false,
                "page" => true,
                _ => continue,
            };
            // The stored lists are these rows: their raw rows go.
            section.rows.retain(|r| {
                !["user_props", "user_prop_defs"].iter().any(|m| {
                    r.path == *m
                        || r.path.starts_with(&format!("{m}["))
                        || r.path.starts_with(&format!("{m}."))
                })
            });
            let Some(model) = section.rows.first().cloned() else {
                continue;
            };
            let row = |key: String, label: String, value: RowValue| PropRow {
                key: format!("{}/{OWN}{key}", section.kind),
                path: format!("{OWN}{key}"),
                depth: 0,
                ty: FieldType::Text,
                value,
                actions: vec![RowAction::Edit],
                route: EditRoute::FieldsSet {
                    kind: section.kind.clone(),
                    path: "user_props".to_owned(),
                },
                identifier: false,
                source: None,
                named_reference: false,
                label: Some(label),
                link: None,
                note: None,
                words: Vec::new(),
                ..model.clone()
            };
            let mut rows = Vec::new();
            for field in project.session.own_field_values(page, &section.targets) {
                let texts: Vec<String> = field
                    .values
                    .into_iter()
                    .map(|v| words(ew_api::TextArg::Ml(v)))
                    .collect();
                let value = match texts.as_slice() {
                    [] => RowValue::Absent,
                    [first, rest @ ..] if rest.iter().all(|t| t == first) => RowValue::One {
                        json: Value::String(first.clone()),
                        text: first.clone(),
                    },
                    many => RowValue::Varies {
                        count: many.len(),
                        texts: many.to_vec(),
                    },
                };
                rows.push(row(field.key, words(ew_api::TextArg::Ml(field.label)), value));
            }
            if !page {
                let mut new = row(
                    "new".to_owned(),
                    strings::text(key!("action-field-new")).to_owned(),
                    RowValue::Absent,
                );
                new.key = OWN_NEW_ROW.to_owned();
                rows.push(new);
            }
            section.rows.extend(rows);
        }
    }

    /// A value typed into a field's row, in the window's language: its project's or pages' value lists written whole
    /// (`fields.set_many` of `user_props`, `ew_api::output::typed_own_values`), an empty text removing the value; the row New field of your own… opens its form. `None` for
    /// another row.
    pub(super) fn commit_own_field(&mut self, key: &str, text: &str) -> Option<Outcome> {
        if key == OWN_NEW_ROW {
            self.props.edit = None;
            return Some(self.act_field_new(&Map::new()));
        }
        let field = own_key(key)?.to_owned();
        let (section, _) = self.props_find(key)?;
        let edits = self.project.as_ref()?.session.own_field_edits(
            section.kind == "page",
            &section.targets,
            &field,
            text,
            // The window's language, as the row shows the value.
            Some(strings::rule_language()),
        );
        let cmd = AnyCommand {
            name: "fields.set_many".to_owned(),
            params: json!({ "edits": edits }),
        };
        let out = self.run_commit(&cmd);
        match &out {
            Outcome::Failed { message } => self.error_under_row(key.to_owned(), message.clone()),
            _ => {
                self.props.edit = None;
                self.props.error = None;
            }
        }
        Some(out)
    }

    /// `field.new {name, for, value}` (New field of your own…): without a name the window opens its form (Name, One
    /// value for: the whole project or each page, Value now); Create field makes the field (`user_prop.define`), its
    /// value the project's, or the focused page's for a field of each page.
    pub(crate) fn act_field_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(n) => n.map(str::trim).unwrap_or_default().to_owned(),
        };
        let scope = match string_param(p, "for") {
            Err(e) => return e,
            Ok(Some("each_page")) => "page",
            Ok(Some("whole_project") | None) => "project",
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["for"]),
        };
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(v) => v.map(str::trim).unwrap_or_default().to_owned(),
        };
        let in_form = self.in_form_of(FIELD_NEW);
        if name.is_empty() && self.mode == Mode::Gui && !in_form {
            return self.open_dialog(Dialog::Form(new_field_form()));
        }
        if name.is_empty() && self.mode == Mode::Headless {
            return failed(key!("error-missing-param"), &["name"]);
        }
        // Asked from a title block's cell (its Shows): the field and the cell's value in one step.
        if let Some(out) = self.frame_field_from_cell(&name, scope, &value) {
            return match out {
                Outcome::Failed { message } if in_form => self.form_failed(FIELD_NEW, message),
                Outcome::Done => {
                    if in_form {
                        self.dialog = None;
                    }
                    Outcome::Done
                }
                other => other,
            };
        }
        let mut params = json!({"label": name, "scope": scope});
        if !value.is_empty() {
            params["value"] = Value::String(value);
            if scope == "page" {
                let Some(page) = self.focused_page_id() else {
                    return Outcome::refused(key!("reason-no-page"));
                };
                params["page"] = Value::String(page.to_string());
            }
        }
        let cmd = AnyCommand {
            name: "user_prop.define".to_owned(),
            params,
        };
        match self.run_commit(&cmd) {
            Outcome::Failed { message } if in_form => self.form_failed(FIELD_NEW, message),
            Outcome::Done => {
                if in_form {
                    self.dialog = None;
                }
                Outcome::Done
            }
            other => other,
        }
    }

    /// `field.rename {key, name}` (a field's Rename…): without a name the window opens a form with its name now.
    pub(crate) fn act_field_rename(&mut self, p: &Map<String, Value>) -> Outcome {
        let in_form = self.in_form_of(FIELD_RENAME);
        let field = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            // The open form renames the field it was opened for.
            Ok(None) => match self.field_renamed.clone() {
                Some(f) if in_form => f,
                _ => return failed(key!("error-missing-param"), &["key"]),
            },
        };
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(n) => n.map(str::trim).unwrap_or_default().to_owned(),
        };
        if name.is_empty() && self.mode == Mode::Gui && !in_form {
            let now = self.project.as_ref().and_then(|project| {
                [false, true]
                    .into_iter()
                    .flat_map(|page| project.session.own_field_values(page, &[]))
                    .find(|f| f.key == field)
                    .map(|f| words(ew_api::TextArg::Ml(f.label)))
            });
            let Some(now) = now else {
                return Outcome::refused(key!("reason-field-gone"));
            };
            self.field_renamed = Some(field);
            return self.open_dialog(Dialog::Form(FormDialog {
                action: FIELD_RENAME,
                title_key: key!("action-field-rename"),
                ok_key: key!("dialog-apply"),
                fields: vec![text_field("name", key!("form-field-name"), now)],
                error: None,
                table: None,
            }));
        }
        let cmd = AnyCommand {
            name: "user_prop.rename".to_owned(),
            params: json!({"key": field, "label": name}),
        };
        match self.run_commit(&cmd) {
            Outcome::Failed { message } if in_form => self.form_failed(FIELD_RENAME, message),
            Outcome::Done => {
                if in_form {
                    self.dialog = None;
                    self.field_renamed = None;
                }
                Outcome::Done
            }
            other => other,
        }
    }

    /// `field.remove {key}` (a field's Remove): the field, its definition and every value (`user_prop.remove`), refused in
    /// words while a frame or title block prints it.
    pub(crate) fn act_field_remove(&mut self, p: &Map<String, Value>) -> Outcome {
        let field = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
        };
        self.run_commit(&AnyCommand {
            name: "user_prop.remove".to_owned(),
            params: json!({ "key": field }),
        })
    }
}

/// A text field of a form.
fn text_field(name: &'static str, label_key: crate::strings::Key<'static>, value: String) -> FormField {
    FormField {
        label: None,
        name: name.into(),
        label_key,
        choices: &super::choices::NONE,
        options: Vec::new(),
        value,
    }
}

/// New field of your own…'s form: its name, whether it has one value for the whole project or one for each page, and its
/// value now.
pub(super) fn new_field_form() -> FormDialog {
    FormDialog {
        action: FIELD_NEW,
        title_key: key!("form-field-new-title"),
        ok_key: key!("dialog-create-field"),
        fields: vec![
            text_field("name", key!("form-field-name"), String::new()),
            FormField {
                choices: &super::choices::FIELD_SCOPES,
                ..text_field("for", key!("form-field-for"), "whole_project".to_owned())
            },
            text_field("value", key!("form-field-value"), String::new()),
        ],
        error: None,
        table: None,
    }
}
