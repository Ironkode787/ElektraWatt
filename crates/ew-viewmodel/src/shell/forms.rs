// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The small forms' machinery: a form's fields and its table ([`FormDialog`], [`FormField`], [`FormTable`]), a form's
//! spec (`Spec`: its action, its title and its fields), the parameters a form sends, typing into a field and the fields
//! that follow it, Enter (`submit_form`: the action with the fields, a refusal kept under them), and the commit path of
//! a create (`host_create`: a preview, then the host's commit).
//!
//! Not here: the forms themselves (New page…, New section…, the settings forms, …) and their actions are in
//! `shell/creation.rs`; other dialogs (the part dialog, the print dialog) in their own files.

use ew_api::{AnyCommand, DynCommand};
use serde_json::{Map, Value};

use super::choices::ClosedChoices;
use super::creation::{MEDIUM_NEW, PAGE_GRID_FIT, PAGE_NEW, medium_preset};
use super::{Dialog, Mode, Outcome, ShellModel, failed};
use crate::key;
use crate::strings;
use crate::strings::Key;
use crate::tools::PreviewState;

/// One field of a creation action: its parameter, the key of its label, the values a choice field offers (none for
/// free text) and the value the dialog starts with.
#[derive(Clone, Copy)]
pub(super) struct Field {
    pub(super) name: &'static str,
    pub(super) label_key: Key<'static>,
    pub(super) choices: &'static ClosedChoices,
    pub(super) default: &'static str,
}

/// A creation action's form: its action, the title key and the fields in order.
pub(super) struct Spec {
    pub(super) action: &'static str,
    pub(super) title_key: Key<'static>,
    pub(super) fields: &'static [Field],
}

pub(super) const fn field(name: &'static str, label_key: Key<'static>) -> Field {
    Field {
        name,
        label_key,
        choices: &super::choices::NONE,
        default: "",
    }
}

/// One field of an open creation dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormField {
    /// The action parameter the field carries: a fixed word, or a path made at run time (a title block's field of the
    /// project's own, `page.prop.<key>`).
    pub name: std::borrow::Cow<'static, str>,
    /// The field's label in the person's own words (a field of the project's own): it stands for `label_key`'s.
    pub label: Option<String>,
    /// The key of the field's label.
    pub label_key: Key<'static>,
    /// The values a choice field offers, in order; empty for a free-text field.
    pub choices: &'static ClosedChoices,
    /// A choice list built at run time (the project's locations, the enclosure parts),
    /// each `(value, words)`: the action receives the value, the window shows the words. Empty for every other field.
    pub options: Vec<(String, String)>,
    /// The text of the field.
    pub value: String,
}

impl FormField {
    /// The field's label: the person's own words, else its key's text.
    pub fn label_text(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| strings::text(self.label_key).to_owned())
    }

    /// Whether the field is a yes-or-no choice (`false`, `true`), which the window draws as a check box.
    pub fn is_toggle(&self) -> bool {
        self.choices.values() == ["false", "true"]
    }

    /// The words of a run-time list of one value that is the field's own (Create enclosure's part on a location that
    /// has its enclosure): there is nothing to choose, and the window shows the words as text, not as a list of one.
    pub fn fixed(&self) -> Option<&str> {
        match self.options.as_slice() {
            [(value, words)] if *value == self.value => Some(words),
            _ => None,
        }
    }

    /// Whether the field is free text: no choice and no run-time option list. A form's first focus is its first text
    /// field, else its Primary (doc 16a §6.12).
    pub fn is_text(&self) -> bool {
        self.choices.is_empty() && self.options.is_empty()
    }
}

/// A creation dialog: the action Enter dispatches with the fields, the title key, the fields, and the refusal of the
/// last attempt.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormDialog {
    /// The action the dialog dispatches (`page.new`).
    pub action: &'static str,
    /// The key of the dialog's title.
    pub title_key: Key<'static>,
    /// The key of the button Enter presses (`Create`; the renumber dialogs: `Renumber`).
    pub ok_key: Key<'static>,
    /// The fields in order.
    pub fields: Vec<FormField>,
    /// What the last attempt was refused with, shown under the fields.
    pub error: Option<String>,
    /// The table under the fields (Save as macro's value sets), `None` for a form without one.
    pub table: Option<FormTable>,
}

impl FormDialog {
    /// Whether it is a revision's dialog (approve, release, withdraw): the release gate's list stands under its body,
    /// Enter presses its primary button, and the answer is dated (the window reads the date, the shell never does).
    pub fn revision(&self) -> bool {
        crate::actions::dated(self.action)
    }

    /// The line under field `field`, if it has one (Renumber pages' Start says that the section dividers
    /// keep their numbers).
    pub fn note(&self, field: &str) -> Option<&'static str> {
        match (self.action, field) {
            ("pages.renumber", "start") => Some(strings::text(key!("form-renumber-start-note"))),
            ("cable.from_wires", "type") => Some(strings::text(key!("form-cable-type-note"))),
            // The length can wait.
            ("cable.from_wires" | "bus.cable", "length") => {
                Some(strings::text(key!("form-cable-length-note")))
            }
            // What the chosen check level stops, in the words of the wizard's step.
            ("project.check_level", "level") => self
                .fields
                .iter()
                .find(|f| f.name == "level")
                .and_then(|f| super::level_sentence(&f.value)),
            // What the chosen type is for (`form-page-type-note-<type>`), else,
            // a list being no page type, where lists are made.
            ("page.new", "type") => Some(
                self.fields
                    .iter()
                    .find(|f| f.name == "type")
                    .and_then(|f| {
                        strings::lookup(&format!("form-page-type-note-{}", f.value.replace('_', "-")))
                    })
                    .unwrap_or_else(|| strings::text(key!("form-page-type-note"))),
            ),
            // Add hole here…'s Size says its unit and an example for the kind chosen (`form-hint-feature-size-<kind>`).
            ("layout.feature_add", "size") => self
                .fields
                .iter()
                .find(|f| f.name == "kind")
                .and_then(|f| strings::lookup(&format!("form-hint-feature-size-{}", f.value))),
            // Every other field says what it changes, with an example, where its form has the text
            // `form-hint-<action>-<field>`.
            _ => strings::lookup(&format!("form-hint-{}-{field}", self.action).replace(['.', '_'], "-")),
        }
    }

    /// Whether the field `field` shows for what the form holds now: Label file…'s Delimiter for a CSV only; every other
    /// field always.
    pub fn shows(&self, field: &str) -> bool {
        match (self.action, field) {
            ("labels.columns", "delimiter") => {
                self.fields.iter().any(|f| f.name == "file" && f.value == "csv")
            }
            _ => true,
        }
    }
}

/// An editable table of a form (the value sets of Save as macro, doc 16a section 6.21's look): a named first
/// column and one column per variable. Submit carries it as the JSON parameter `name`: one object per row,
/// `{"name": <first column>, "values": {<column>: <cell>}}`, blank cells left out.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormTable {
    /// The action parameter the table carries (`value_sets`).
    pub name: &'static str,
    /// The key of the table's heading (`Value sets`).
    pub heading_key: Key<'static>,
    /// Whether the rows are given (the Cable form's wires): no row is added or removed and the first column is not
    /// edited.
    pub fixed: bool,
    /// The key of the first column's header (`Value set`).
    pub key_label: Key<'static>,
    /// The key of the button under a table that is not fixed, which adds a row (`Add value set`).
    pub add_key: Key<'static>,
    /// The columns after the first: the variable's name and its type's word (`number`), which types the cells.
    pub columns: Vec<(String, String)>,
    /// The rows in order.
    pub rows: Vec<FormRow>,
}

/// The type word of a [`FormTable`] column of ticks (Give location's devices): a cell is a tick box, [`TICKED`] when
/// ticked, empty when not.
pub const TICK: &str = "tick";

/// The text of a ticked cell of a [`TICK`] column.
pub const TICKED: &str = "yes";

/// One row of a [`FormTable`]: the first column's text and one cell text per column.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormRow {
    /// The first column's text (a value set's name).
    pub key: String,
    /// One text per column of the table, in order.
    pub cells: Vec<String>,
}

impl FormTable {
    /// The table as its action parameter (see [`FormTable`]).
    fn param(&self) -> Value {
        Value::Array(
            self.rows
                .iter()
                .map(|r| {
                    let values: Map<String, Value> = self
                        .columns
                        .iter()
                        .zip(&r.cells)
                        .filter(|(_, cell)| !cell.trim().is_empty())
                        .map(|((col, _), cell)| (col.clone(), Value::String(cell.trim().to_owned())))
                        .collect();
                    serde_json::json!({ "name": r.key.trim(), "values": values })
                })
                .collect(),
        )
    }
}

impl Spec {
    /// The dialog of this form, its fields taken from the parameters given (else their defaults).
    pub(super) fn dialog(&self, p: &Map<String, Value>) -> FormDialog {
        FormDialog {
            action: self.action,
            title_key: self.title_key,
            ok_key: key!("dialog-create"),
            fields: self
                .fields
                .iter()
                .map(|f| FormField {
                    label: None,
                    name: f.name.into(),
                    label_key: f.label_key,
                    choices: f.choices,
                    options: Vec::new(),
                    value: text_param(p, f.name)
                        .ok()
                        .filter(|v| !v.is_empty())
                        .unwrap_or_else(|| f.default.to_owned()),
                })
                .collect(),
            error: None,
            table: None,
        }
    }
}

/// A text parameter: a string (a number is its decimal text), `""` when it is absent or null; `Err` for any other type.
pub(super) fn text_param(p: &Map<String, Value>, name: &str) -> Result<String, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s.trim().to_owned()),
        Some(Value::Number(n)) => Ok(n.to_string()),
        Some(_) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// A text parameter of a settings form, which a call may leave out: `None` when it is absent or null (the stored value
/// stays), else its text (empty clears the value); `Err` for any other type.
pub(super) fn given_text(p: &Map<String, Value>, name: &str) -> Result<Option<String>, Outcome> {
    match p.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => text_param(p, name).map(Some),
    }
}

/// A yes-or-no parameter of a settings form (`true`, `"true"`, `false`, `"false"`): `None` when it is absent.
pub(super) fn given_flag(p: &Map<String, Value>, name: &str) -> Option<bool> {
    match p.get(name) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => Some(s == "true"),
        _ => None,
    }
}

/// Doc 16a §8 item 5: a form's common refusal reasons, each with the key of what to do next.
const NEXT_STEPS: [(Key<'static>, Key<'static>); 3] = [
    (key!("reason-read-only"), key!("form-step-read-only")),
    (key!("reason-no-page"), key!("form-step-no-page")),
    (
        key!("reason-no-devices-selected"),
        key!("form-step-no-devices-selected"),
    ),
];

/// The text a refused form shows: the reason, and what to do next where [`NEXT_STEPS`] has it.
fn refusal_text(reason: &ew_api::Text) -> String {
    match NEXT_STEPS.iter().find(|(r, _)| reason == r) {
        Some(&(_, step)) => strings::format(
            key!("form-refused-step"),
            &[&strings::render(reason), strings::text(step)],
        ),
        None => strings::render(reason),
    }
}

impl ShellModel {
    /// Sets the text of field `name` of the open creation dialog (a keystroke or a choice) and clears the last refusal.
    pub fn set_form_field(&mut self, name: &str, value: &str) {
        // New cable type…, the last entry of a Cable form's list of types, opens Parts on a new cable; Save comes
        // back with the new type chosen.
        if name == "part"
            && value == super::cables::NEW_CABLE_TYPE
            && matches!(&self.dialog, Some(Dialog::Form(f)) if super::cables::chooses_cable_type(f))
        {
            let _ = self.act_parts_new(&Map::from_iter([("start".to_owned(), Value::from("cable"))]));
            return;
        }
        // A bus line's Cable form: New bus cable… and New plug… open Parts on a new cable or plug for the
        // line's bus; another cable chosen shows its tag.
        if let Some(Dialog::Form(f)) = &self.dialog
            && f.action == super::cables::BUS_CABLE
        {
            if value == super::cables::NEW_BUS_PART && ["part", "plug_a", "plug_b"].contains(&name) {
                let lines = self.selected_bus_lines();
                let bus = self
                    .project
                    .as_ref()
                    .map(|p| p.session.bus_cable_form(&lines).system)
                    .unwrap_or_default();
                let start = if name == "part" { "cable" } else { "plug" };
                let params = Map::from_iter([
                    ("start".to_owned(), Value::from(start)),
                    ("bus".to_owned(), Value::String(bus)),
                    ("field".to_owned(), Value::String(name.to_owned())),
                ]);
                let _ = self.act_parts_new(&params);
                return;
            }
            if name == "cable" {
                self.bus_cable_chosen(value);
            }
        }
        let cable_rows = (["type", "part"].contains(&name)
            && matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == "cable.from_wires"))
        .then(|| self.cable_row_wires());
        // Create enclosure's code of a new location comes and goes with its "New location" choice, and its part
        // choices follow the location: the part of its enclosure alone, where it has one.
        let code = (name == "location").then(|| self.proposed_location_code());
        let enclosure_parts = (name == "location" && self.in_form_of("layout.enclosure_new"))
            .then(|| self.enclosure_part_choices(value));
        // The grid question counts again for another grid.
        if name == "grid" && self.in_form_of(PAGE_GRID_FIT.action) {
            self.refresh_grid_fit(value);
        }
        // New page's folio follows its section to the section's first free folio (it stayed 3 for section 2), its
        // kind and its place to the proposal for them unless a number was typed.
        let free = (name == "section" && self.in_form_of(PAGE_NEW.action))
            .then(|| self.section_free_folio(value))
            .flatten();
        let refolio = if matches!(name, "type" | "place") && self.in_form_of(PAGE_NEW.action) {
            self.page_refolio(name, value)
        } else {
            None
        };
        if let Some(Dialog::Form(form)) = &mut self.dialog {
            if let Some(f) = form.fields.iter_mut().find(|f| f.name == name) {
                f.value = value.to_owned();
            }
            form.error = None;
            // Cable…'s cores follow its type; a cable part chosen is the type.
            if let Some(wires) = &cable_rows {
                if name == "part"
                    && !value.is_empty()
                    && let Some(t) = form.fields.iter_mut().find(|f| f.name == "type")
                {
                    t.value = value.to_owned();
                }
                // A size typed is no part: the list says so.
                if name == "type"
                    && let Some(p) = form.fields.iter_mut().find(|f| f.name == "part")
                    && p.value != value
                {
                    p.value = String::new();
                }
                super::cables::retype(self.project.as_ref().map(|p| &p.session), wires, form);
            }
            if form.action == "layout.enclosure_new"
                && let Some(code) = code
            {
                super::layout::new_location_field(form, &code);
            }
            if let Some((options, held)) = enclosure_parts
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == "part")
            {
                let kept = options.iter().any(|(v, _)| *v == f.value);
                f.value = held
                    .or_else(|| kept.then(|| f.value.clone()))
                    .or_else(|| options.first().map(|(v, _)| v.clone()))
                    .unwrap_or_default();
                f.options = options;
            }
            if name == "system" {
                super::supply_place::own_names_field(form);
            }
            // Copy pages takes a folio offset or a folio to go after, never both: typing into one clears the other.
            if form.action == super::page_copy::ACTION
                && !value.trim().is_empty()
                && let Some(other) = match name {
                    "offset" => Some("after"),
                    "after" => Some("offset"),
                    _ => None,
                }
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == other)
            {
                f.value.clear();
            }
            if let Some(free) = free
                && let Some(f) = form.fields.iter_mut().find(|f| f.name == "folio")
            {
                f.value = free.to_string();
            }
            if let Some((folio, section)) = &refolio {
                for f in &mut form.fields {
                    match &*f.name {
                        "folio" => f.value.clone_from(folio),
                        "section" => f.value.clone_from(section),
                        _ => {}
                    }
                }
            }
            // New medium…'s usual medium fills its name and pressure, both still to be changed.
            if form.action == MEDIUM_NEW.action
                && name == "preset"
                && let Some((words, p_nom)) = medium_preset(value)
            {
                for f in &mut form.fields {
                    match &*f.name {
                        "name" => f.value.clone_from(&words),
                        "p_nom" => p_nom.clone_into(&mut f.value),
                        _ => {}
                    }
                }
            }
        }
        // Terminal…'s Kind and Part follow each other.
        if self.in_form_of("tool.place_terminal") {
            self.terminal_form_follow(name);
        }
        if let Some((folio, _)) = refolio {
            let at = self.tools.page_proposal.as_ref().and_then(|p| p.1.clone());
            self.tools.page_proposal = Some((folio, at));
            self.page_place_options();
        } else if matches!(name, "folio" | "section") && self.in_form_of(PAGE_NEW.action) {
            // The Place choice names the page the folio in the field stands after, as the line under it does.
            self.page_place_options();
        }
    }

    /// Sets the first-column text (`column` `None`, a rename) or a cell of row `row` of the open form's table, and
    /// clears the last refusal; a row or column the table does not have changes nothing.
    pub fn set_form_table_cell(&mut self, row: usize, column: Option<usize>, value: &str) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
            && let Some(r) = table.rows.get_mut(row)
        {
            let slot = match column {
                None => Some(&mut r.key),
                Some(c) => r.cells.get_mut(c),
            };
            if let Some(slot) = slot {
                value.clone_into(slot);
                form.error = None;
            }
        }
    }

    /// Adds a row to the open form's table, its cells blank: a value set named `Set <n>` with the lowest `n` from the
    /// row count up that no row has; a row of another table (Copy pages…'s texts to change) with its first cell blank
    /// too, to be filled (a name there read as a text to change).
    pub fn add_form_table_row(&mut self) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
        {
            let key = if table.name == "value_sets" {
                (table.rows.len() + 1..)
                    .map(|n| strings::format(key!("form-value-set-new"), &[&n.to_string()]))
                    .find(|k| table.rows.iter().all(|r| r.key.trim() != k))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            table.rows.push(FormRow {
                key,
                cells: vec![String::new(); table.columns.len()],
            });
            form.error = None;
        }
    }

    /// Removes row `row` of the open form's table; a row it does not have changes nothing.
    pub fn remove_form_table_row(&mut self, row: usize) {
        if let Some(Dialog::Form(form)) = &mut self.dialog
            && let Some(table) = &mut form.table
            && row < table.rows.len()
        {
            table.rows.remove(row);
            form.error = None;
        }
    }

    /// Enter in the creation dialog (or its Create button): the dialog's action with its non-empty fields as parameters
    /// (and its table's rows). A refusal stays in the dialog, its reason shown in the form;
    /// success closes it.
    pub fn submit_form(&mut self) -> Outcome {
        let Some(Dialog::Form(form)) = &self.dialog else {
            return Outcome::Done;
        };
        let action = form.action;
        let mut params: Map<String, Value> = form
            .fields
            .iter()
            .filter(|f| !f.value.trim().is_empty())
            .map(|f| (f.name.to_string(), Value::String(f.value.trim().to_owned())))
            .collect();
        if let Some(table) = &form.table {
            params.insert(table.name.to_owned(), table.param());
        }
        let out = self.dispatch(action, &Value::Object(params));
        // A refused submit says why under the fields, as a failed one does.
        if let Outcome::Refused { reason: reason_key } = &out
            && let Some(Dialog::Form(f)) = &mut self.dialog
            && f.action == action
        {
            f.error = Some(refusal_text(reason_key));
        }
        // The project closed under the open dialog: nothing is left to create in.
        if matches!(out, Outcome::Refused { .. }) && !self.has_project() {
            self.dialog = None;
        }
        out
    }

    /// A required parameter is missing: in the window the dialog opens (or says what is missing), headless the step
    /// fails.
    pub(super) fn form_missing(
        &mut self,
        spec: &'static Spec,
        p: &Map<String, Value>,
        name: &str,
    ) -> Outcome {
        match self.mode {
            Mode::Headless => failed(key!("error-missing-param"), &[name]),
            Mode::Gui if self.in_form_of(spec.action) => {
                self.form_failed(spec.action, self.missing_text(name))
            }
            Mode::Gui => self.open_dialog(Dialog::Form(spec.dialog(p))),
        }
    }

    /// The text of a missing parameter `name`: by its field's label when the open form has the field, else
    /// by the parameter's name.
    pub(super) fn missing_text(&self, name: &str) -> String {
        if let Some(Dialog::Form(f)) = &self.dialog
            && let Some(field) = f.fields.iter().find(|f| f.name == name)
        {
            return strings::format(key!("error-missing-field"), &[strings::text(field.label_key)]);
        }
        strings::format(key!("error-missing-param"), &[name])
    }

    /// One command through the host with a preview (module documentation): `Session::preview_with` once, kept as the
    /// host keeps a tool's preview, then the host's commit. A refusal of the preview is the outcome's message.
    pub(super) fn host_create(&mut self, name: &str, params: Value) -> Outcome {
        self.host_create_noticed(name, params).0
    }

    /// [`ShellModel::host_create`] with the notices of the command's outcome (what it left out instead of refusing),
    /// read on its preview (Auto-fill rail's toast).
    pub(super) fn host_create_noticed(&mut self, name: &str, params: Value) -> (Outcome, Vec<String>) {
        let cmd = AnyCommand {
            name: name.into(),
            params,
        };
        let json = match cmd.to_json() {
            Ok(v) => v,
            Err(e) => return (super::tools::command_failed(&e, &cmd.params), Vec::new()),
        };
        let refused = || (Outcome::refused(key!("reason-no-project")), Vec::new());
        let Some(base_rev) = self.revision() else {
            return refused();
        };
        let submit = self.tool_submit();
        let Some(project) = &mut self.project else {
            return refused();
        };
        let result = project.session.preview_with(&cmd, submit);
        self.count(|c| c.preview_with += 1);
        let preview = match result {
            Ok(p) => p,
            Err(e) => {
                let out = super::tools::command_failed(&e, &json);
                return (self.command_refused(&e, out), Vec::new());
            }
        };
        let notices = super::tools::notices_words(&preview.outcome().notices);
        // A tool's own kept preview (the armed Place tool's ghost) is put back: it is stale after the commit and the
        // tool computes it again, as it would without this action.
        let tool_preview = self.tools.preview.replace(PreviewState {
            base_rev,
            command: json,
            name: name.to_owned(),
            result: Ok(preview),
            derived: Default::default(),
        });
        let out = self.run_commit(&cmd);
        self.tools.preview = tool_preview;
        (out, notices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refused form says what to do next for a read-only project and for no page shown, in either
    /// language.
    #[test]
    fn a_refused_form_names_the_next_step() {
        let texts = || [key!("reason-read-only"), key!("reason-no-page")].map(|k| refusal_text(&k.into()));
        assert_eq!(
            texts(),
            [
                "The project is open read-only. File ▸ Save as… makes a copy that can be changed.",
                "No page is shown. Open a page, or choose another scope.",
            ]
        );
        assert!(strings::set_locale("nl"));
        assert_eq!(
            texts(),
            [
                "Het project is alleen-lezen geopend. Bestand ▸ Opslaan als… maakt een kopie die u kunt wijzigen.",
                "Er wordt geen pagina getoond. Open een pagina, of kies een ander bereik.",
            ]
        );
        assert!(strings::set_locale("en"));
    }
}
