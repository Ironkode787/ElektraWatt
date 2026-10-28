// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid in the shell (WP-S2.05 part 1, spec §6.2 to §6.11): the sheet fetched once per key, the row
//! cursor, the edit buffer, the commit path, the card, the toast and `select.set`.
//!
//! **The sheet** is fetched at the end of a dispatch when the pane is in the layout and its key (project session,
//! revision, the records) changed: a selection change, a focused-page change with an empty selection, or a new
//! revision makes exactly one `Session::property_sheet` call; row moves, category toggles, typing, choices and popups
//! make none (§9 item 4). With the pane closed there is no sheet and no call.
//!
//! **The commit path** (D6, D7): an edit compiles into exactly one command (`fields.set` or `project.settings.set`),
//! refused unless its exposure is `Edit` (D4). It is previewed once (`Session::preview_with`); the edit policy decides
//! on the preview: a direct edit commits the kept preview (the window: `Session::commit`; headless: a one-line command
//! log through `Session::run_script`, as `command.run`), any other shows the card. A stale card is never re-previewed
//! with its old parameters: Apply checks the edited member against the base value and refuses
//! `reason-changed-meanwhile` when it moved, else rebuilds the edit on the current values and shows a fresh card.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::output::ew_dl::{Destination, EntityRef};
use ew_api::output::{InputError, ParsedInput};
use ew_api::{AnyCommand, CmdError, Exposure, Submit};
use serde_json::{Map, Value, json};

use super::navigation::{Focus, bump};
use super::{Mode, Outcome, RecordedCommand, ShellModel, failed, string_param};
use crate::properties::{
    Card, CardOrigin, ChoiceKind, Choices, Decision, EditBuffer, PendingEdit, Popup, PropRow,
    PropertiesState, PropertySheet, Provenance, RowAction, RowValue, SheetKey, SheetSection, Toast, compose,
    find_row, is_derived, item_default, kind_plural, member_path, policy, read_only_reason, visible_rows,
};
use crate::strings;
use crate::tools::PreviewState;

/// The message of a command refusal as the grid shows it (§6.7): a parameter path `/targets/<i>` names the target by
/// its label (`label(i)`, "on -101F1: …").
fn refusal_text(e: &CmdError, label: &dyn Fn(usize) -> String) -> String {
    match e {
        CmdError::InvalidParams { path, message, .. } => {
            match path
                .strip_prefix("/targets/")
                .and_then(|i| i.parse::<usize>().ok())
            {
                Some(i) => strings::format("props-refused-on", &[&label(i), message]),
                None => message.clone(),
            }
        }
        CmdError::Rejected { reason, .. } => reason.clone(),
        CmdError::Prevented { rule, finding, .. } => format!("{rule}: {finding}"),
        CmdError::NotFound { .. } => strings::text("props-gone").to_owned(),
        other => other.to_string(),
    }
}

/// `text` with every hyphenated UUID replaced by `name(uuid)` (no UUID reaches the transcript, §9 item 5).
pub(super) fn replace_uuids(text: &str, name: &dyn Fn(&str) -> String) -> String {
    let b = text.as_bytes();
    let is_uuid = |w: &[u8]| {
        w.len() == 36
            && w.iter().enumerate().all(|(k, c)| match k {
                8 | 13 | 18 | 23 => *c == b'-',
                _ => c.is_ascii_hexdigit(),
            })
    };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if i + 36 <= b.len() && is_uuid(&b[i..i + 36]) && text.is_char_boundary(i + 36) {
            out.push_str(&name(&text[i..i + 36]));
            i += 36;
        } else {
            let ch = text[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

impl ShellModel {
    /// A command refusal as the grid shows it: [`refusal_text`] with the target named by its label (the header's title
    /// for one target, else the title of that record's own sheet: one `Session::property_sheet` call, on a refusal
    /// only), page IDs as folios and other IDs left out.
    fn refusal(&self, e: &CmdError, section: Option<&SheetSection>) -> String {
        let rows = self.page_rows();
        let label = |i: usize| -> String {
            let target = section.and_then(|s| s.targets.get(i).copied());
            let single = section.is_some_and(|s| s.targets.len() == 1);
            match (target, &self.project) {
                (Some(_), _) if single => self
                    .props
                    .sheet()
                    .map_or(String::new(), |s| s.header.title.clone()),
                (Some(t), Some(p)) => {
                    bump(&self.calls, |c| c.property_sheet += 1);
                    p.session.property_sheet(&[t]).header.title
                }
                _ => strings::format("props-target-n", &[&(i + 1).to_string()]),
            }
        };
        replace_uuids(&refusal_text(e, &label), &|u| {
            rows.iter()
                .find(|r| r.id.to_string() == u)
                .map_or_else(|| strings::text("props-a-record").to_owned(), |r| r.folio.clone())
        })
    }

    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The property grid's state.
    pub fn properties(&self) -> &PropertiesState {
        &self.props
    }

    /// Whether the property pane is in the layout of the project view.
    pub fn properties_shown(&self) -> bool {
        self.has_project()
            && self.wizard().is_none()
            && self.first_run().is_none()
            && self
                .layout()
                .panes()
                .contains(&crate::layout::PaneKind::Properties)
    }

    /// The collapsed categories (`device/identity`), from the settings file (`properties.collapsed`).
    pub fn properties_collapsed(&self) -> BTreeSet<String> {
        self.settings.properties_collapsed.iter().cloned().collect()
    }

    /// The focused row: the row the cursor was moved to while it is on show, else the first row on show.
    pub fn properties_row(&self) -> Option<String> {
        let sheet = self.props.sheet()?;
        let collapsed = self.properties_collapsed();
        let rows = visible_rows(sheet, &collapsed);
        if let Some(moved) = self.props.moved_row()
            && rows.iter().any(|r| r.row.key == moved)
        {
            return Some(moved.to_owned());
        }
        rows.first().map(|r| r.row.key.clone())
    }

    /// The records the sheet is for: the project in `props.project` mode, else the selection made on the page on show,
    /// else the focused page. A page change leaves the selection in the shell (it survives tabs and pages), but the pane
    /// shows its own sheet again until the selection is made on the new page (window check, item 6).
    fn sheet_ids(&self) -> Vec<EntityRef> {
        let Some(p) = &self.project else {
            return Vec::new();
        };
        let generation = self.tools.selection.generation;
        if self.props.project_mode == Some(generation) {
            return vec![EntityRef(p.session.query().project().id.as_uuid())];
        }
        let made_here = self
            .props
            .selection_page
            .as_ref()
            .is_some_and(|(g, page)| *g == generation && *page == self.current_page());
        if !self.tools.selection.is_empty() && made_here {
            return self.tools.selection.items.clone();
        }
        match self.current_page().map(|id| id.0) {
            Some(Destination::Page(u)) => vec![EntityRef(u)],
            _ => Vec::new(),
        }
    }

    /// Fetches the sheet when the pane is shown and its key changed (the end of every dispatch, a layout change).
    pub(super) fn sync_properties(&mut self) {
        // The page a selection was made on: a page change with nothing selected there shows the page's own sheet
        // (window check, item 6).
        let generation = self.tools.selection.generation;
        if self
            .props
            .selection_page
            .as_ref()
            .is_none_or(|(g, _)| *g != generation)
        {
            self.props.selection_page = Some((generation, self.current_page()));
        }
        if !self.properties_shown() {
            self.props.sheet = None;
            self.props.key = None;
            return;
        }
        let (Some(epoch), Some(rev)) = (self.project_epoch(), self.revision()) else {
            return;
        };
        let key = SheetKey {
            epoch,
            rev,
            ids: self.sheet_ids(),
        };
        if self.props.key.as_ref() == Some(&key) {
            return;
        }
        let same_records = self
            .props
            .key
            .as_ref()
            .is_some_and(|k| k.epoch == key.epoch && k.ids == key.ids);
        if !same_records {
            // Another selection: an open edit, its choices, a popup and the card go; the row cursor follows.
            self.props.edit = None;
            self.props.error = None;
            self.props.choices = None;
            self.props.popup = None;
            // The grid's card goes; a replace card is the find pane's and does not follow the selection (WP-S2.05 part 2).
            if self
                .props
                .card
                .as_ref()
                .is_some_and(|c| matches!(c.origin, CardOrigin::Properties))
            {
                self.props.card = None;
            }
            self.props.row = None;
            self.props.notes.clear();
        }
        self.props.sheet = Some(Arc::new(self.fetch_sheet(&key.ids)));
        self.props.key = Some(key);
    }

    /// One `Session::property_sheet` call, counted, with the shell's read-only reason.
    fn fetch_sheet(&self, ids: &[EntityRef]) -> PropertySheet {
        let Some(p) = &self.project else {
            return PropertySheet {
                rev: 0,
                header: Default::default(),
                sections: Vec::new(),
                missing: 0,
                read_only: None,
                languages: Vec::new(),
            };
        };
        bump(&self.calls, |c| c.property_sheet += 1);
        let uuids: Vec<_> = ids.iter().map(|e| e.0).collect();
        let mut sheet = p.session.property_sheet(&uuids);
        sheet.read_only = self.read_only_reason().map(str::to_owned);
        // A read-only project's rows offer nothing (§6.3, §6.8).
        if sheet.read_only.is_some() {
            for s in &mut sheet.sections {
                for r in &mut s.rows {
                    r.actions.clear();
                }
            }
        }
        sheet
    }

    /// The row named by `row` in `p`, else the focused row.
    fn props_row_param(&self, p: &Map<String, Value>) -> Result<String, Outcome> {
        match string_param(p, "row")? {
            Some(r) => {
                let sheet = self.props.sheet().ok_or(Outcome::Refused {
                    reason_key: "reason-no-sheet",
                })?;
                if find_row(sheet, r).is_none() {
                    return Err(failed("error-no-row", &[r]));
                }
                Ok(r.to_owned())
            }
            None => self.properties_row().ok_or(Outcome::Refused {
                reason_key: "reason-no-row",
            }),
        }
    }

    /// The section and row of `key` in the current sheet, cloned.
    fn props_find(&self, key: &str) -> Option<(SheetSection, PropRow)> {
        let sheet = self.props.sheet()?;
        find_row(sheet, key).map(|(s, r)| (s.clone(), r.clone()))
    }

    /// Why a row cannot be edited now: `None` when it can.
    fn props_refusal(
        &self,
        section: &SheetSection,
        row: &PropRow,
        action: RowAction,
    ) -> Option<&'static str> {
        let sheet = self.props.sheet()?;
        if sheet.read_only.is_some() || !self.is_writable() {
            return Some("reason-read-only");
        }
        if row.actions.contains(&action) {
            return None;
        }
        if read_only_reason(sheet, section, row).is_some()
            && matches!(
                row.route,
                crate::properties::EditRoute::ReadOnly(crate::properties::FieldReadOnly::HumanOnly { .. })
            )
        {
            return Some("reason-human-only");
        }
        Some("reason-read-only")
    }

    // -----------------------------------------------------------------------------------------------------------
    // View actions

    /// `props.show` (View ▸ Properties): toggles the pane, or sets it with `on`.
    pub(super) fn act_props_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self.layout.panes().contains(&crate::layout::PaneKind::Properties);
        let want = p.get("on").and_then(Value::as_bool).unwrap_or(!shown);
        if want != shown {
            let layout = if want {
                self.layout.with_properties()
            } else {
                self.layout.without(crate::layout::PaneKind::Properties)
            };
            self.replace_layout(layout);
            if !want && self.focus == Some(Focus::Properties) {
                self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
            }
        }
        Outcome::Done
    }

    /// `props.focus` (E, Alt+Enter on the canvas): the pane gets the focus (shown first when closed).
    pub(super) fn act_props_focus(&mut self) -> Outcome {
        if !self.layout.panes().contains(&crate::layout::PaneKind::Properties) {
            let _ = self.act_props_show(&Map::new());
        }
        self.focus = Some(Focus::Properties);
        Outcome::Done
    }

    /// `props.project` (Project properties): the project record until the selection changes.
    pub(super) fn act_props_project(&mut self) -> Outcome {
        self.props.project_mode = Some(self.tools.selection.generation);
        self.act_props_focus()
    }

    /// `select.set`: `ids` (UUID texts) or `refs` (command-log references); an empty list clears; one unresolvable
    /// entry refuses the whole action with the resolver's message.
    pub(super) fn act_select_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let mut items: Vec<EntityRef> = Vec::new();
        match (p.get("ids"), p.get("refs")) {
            (Some(Value::Array(ids)), None) => {
                for v in ids {
                    let Some(text) = v.as_str() else {
                        return failed("error-bad-param", &["ids"]);
                    };
                    match project.session.resolve_reference(&format!("id:{text}")) {
                        Ok(u) => items.push(EntityRef(u)),
                        Err(m) => return Outcome::Failed { message: m },
                    }
                }
            }
            (None, Some(Value::Array(refs))) => {
                for v in refs {
                    let Some(text) = v.as_str() else {
                        return failed("error-bad-param", &["refs"]);
                    };
                    if let Some(ends) = text.strip_prefix("wire:") {
                        match self.wire_of_page(ends) {
                            Ok(e) => items.push(e),
                            Err(out) => return out,
                        }
                        continue;
                    }
                    match project.session.resolve_reference(text) {
                        Ok(u) => items.push(EntityRef(u)),
                        Err(m) => return Outcome::Failed { message: m },
                    }
                }
            }
            (None, None) => return failed("error-missing-param", &["ids"]),
            _ => return failed("error-bad-param", &["ids"]),
        }
        let change = crate::tools::SelectionChange::Replace(items);
        if self.tools.selection.apply(&change) {
            self.tools.announcement = Some(strings::format(
                "announce-selected",
                &[&self.tools.selection.items.len().to_string()],
            ));
        }
        Outcome::Done
    }

    /// The drawn connection of the page shown whose ends the page text names `<a> -> <b>` (either way round; quotes
    /// around an end are optional; FT-485 item 4): a wire whose drawn route another wire or a symbol's body covers
    /// cannot be clicked, so `select.set` takes `wire:<a> -> <b>` beside the session's references. No such wire, or
    /// several, fail with a message.
    fn wire_of_page(&self, ends: &str) -> Result<EntityRef, Outcome> {
        let unquote = |s: &str| s.trim().trim_matches('"').to_owned();
        let Some((a, b)) = ends.split_once("->").map(|(a, b)| (unquote(a), unquote(b))) else {
            return Err(failed("error-wire-reference", &[ends]));
        };
        let Some(c) = self.canvas() else {
            return Err(Outcome::Refused {
                reason_key: "reason-no-page",
            });
        };
        let hits: Vec<EntityRef> = crate::describe::page_wires(c.page())
            .into_iter()
            .filter(|(_, x, y)| (*x == a && *y == b) || (*x == b && *y == a))
            .map(|(e, _, _)| e)
            .collect();
        match hits.as_slice() {
            [one] => Ok(*one),
            [] => Err(failed("error-wire-reference", &[ends])),
            _ => Err(failed("error-wire-ambiguous", &[ends])),
        }
    }

    /// `props.row`: `step` (±n, clamped) or `to` (a row key, `first`, `last`).
    pub(super) fn act_props_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sheet) = self.props.sheet.clone() else {
            return Outcome::Refused {
                reason_key: "reason-no-sheet",
            };
        };
        let collapsed = self.properties_collapsed();
        let rows = visible_rows(&sheet, &collapsed);
        if rows.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-row",
            };
        }
        let here = self
            .properties_row()
            .and_then(|k| rows.iter().position(|r| r.row.key == k))
            .unwrap_or(0);
        let at = if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return failed("error-bad-param", &["step"]);
            };
            (here as i64 + step).clamp(0, rows.len() as i64 - 1) as usize
        } else {
            match string_param(p, "to") {
                Err(e) => return e,
                Ok(Some("first")) => 0,
                Ok(Some("last")) => rows.len() - 1,
                Ok(Some(key)) => match rows.iter().position(|r| r.row.key == key) {
                    Some(i) => i,
                    None => return failed("error-no-row", &[key]),
                },
                Ok(None) => return failed("error-missing-param", &["step"]),
            }
        };
        self.props.row = Some(rows[at].row.key.clone());
        self.props.popup = None;
        Outcome::Done
    }

    /// `props.category`: `category` (its word, of the focused row's section; else the focused row's) and `expanded`
    /// (else it toggles). Collapse states persist per (kind, category) in the settings file.
    pub(super) fn act_props_category(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sheet) = self.props.sheet.clone() else {
            return Outcome::Refused {
                reason_key: "reason-no-sheet",
            };
        };
        let focused = self.properties_row();
        let (section, row) = match focused.as_deref().and_then(|k| find_row(&sheet, k)) {
            Some(x) => x,
            None => match sheet
                .sections
                .first()
                .and_then(|s| s.rows.first().map(|r| (s, r)))
            {
                Some(x) => x,
                None => {
                    return Outcome::Refused {
                        reason_key: "reason-no-row",
                    };
                }
            },
        };
        let category = match string_param(p, "category") {
            Err(e) => return e,
            Ok(Some(word)) => match crate::properties::Category::ALL.iter().find(|c| c.word() == word) {
                Some(c) => *c,
                None => return failed("error-bad-param", &["category"]),
            },
            Ok(None) => row.category,
        };
        let key = crate::properties::collapse_key(&section.kind, category);
        let collapsed = self.settings.properties_collapsed.contains(&key);
        let expand = p.get("expanded").and_then(Value::as_bool).unwrap_or(collapsed);
        if expand {
            self.settings.properties_collapsed.retain(|k| *k != key);
        } else if !collapsed {
            self.settings.properties_collapsed.push(key);
            self.settings.properties_collapsed.sort();
        }
        if expand == collapsed {
            self.save_settings();
        }
        Outcome::Done
    }

    /// `props.edit`: opens the editor of `row` (else the focused row) with its value's text.
    pub(super) fn act_props_edit(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        // A derived row edits its authored partner: Enter or F2 is its Override (§6.4).
        let overrides = self.props_find(&key).is_some_and(|(_, r)| {
            !r.actions.contains(&RowAction::Edit) && r.actions.contains(&RowAction::Override)
        });
        if overrides {
            return self.row_action(&key, RowAction::Override);
        }
        self.begin_edit(&key, RowAction::Edit, None)
    }

    /// Opens the editor of `key` for `action` (Edit or Override) with `buffer`, else the value's text.
    fn begin_edit(&mut self, key: &str, action: RowAction, buffer: Option<String>) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed("error-no-row", &[key]);
        };
        if let Some(reason_key) = self.props_refusal(&section, &row, action) {
            return Outcome::Refused { reason_key };
        }
        let shown = buffer.is_none();
        let buffer = match buffer {
            Some(b) => b,
            None => match (&row.value, self.mltext_object(&section, &row)) {
                // A multi-language value edits its source language's text inside the object (§6.5).
                (_, Some(object)) => self.source_text(&object),
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
        self.props.choices = None;
        self.props.popup = None;
        // A choice editor lists its options (§6.5).
        if crate::properties::has_options(&row.ty) {
            self.props.choices = self.options(&section, &row, "", key);
        }
        Outcome::Done
    }

    /// The options of a choice editor for `row` matching `filter` (`Session::field_choices`).
    fn options(&self, section: &SheetSection, row: &PropRow, filter: &str, key: &str) -> Option<Choices> {
        let p = self.project.as_ref()?;
        bump(&self.calls, |c| c.field_choices += 1);
        let items: Vec<(String, Value)> = p
            .session
            .field_choices(&section.kind, &row.path, filter)
            .into_iter()
            .map(|c| (c.text, c.value))
            .collect();
        Some(Choices {
            row: key.to_owned(),
            items,
            kind: ChoiceKind::Options,
            index: None,
        })
    }

    /// The stored language object of a multi-language row, if its value is one: the row's own value, or a derived
    /// row's authored partner's.
    fn mltext_object(&self, section: &SheetSection, row: &PropRow) -> Option<serde_json::Map<String, Value>> {
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
    fn source_language(&self, object: &serde_json::Map<String, Value>) -> String {
        self.props
            .sheet()
            .and_then(|s| s.languages.first().cloned())
            .or_else(|| object.keys().next().cloned())
            .unwrap_or_else(|| "en".to_owned())
    }

    /// The source language's text of a language object.
    fn source_text(&self, object: &serde_json::Map<String, Value>) -> String {
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
    fn first_target_json(&self, section: &SheetSection, row: &PropRow) -> String {
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
    pub(super) fn act_props_input(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed("error-missing-param", &["text"]),
        };
        if self.props.edit.is_none() {
            let Some(key) = self.properties_row() else {
                return Outcome::Refused {
                    reason_key: "reason-no-row",
                };
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
    pub(super) fn act_props_cancel(&mut self) -> Outcome {
        self.props.edit = None;
        self.props.error = None;
        self.props.choices = None;
        self.props.popup = None;
        Outcome::Done
    }

    /// `props.commit` (Enter, Tab, focus loss): the buffer is parsed and committed.
    pub(super) fn act_props_commit(&mut self) -> Outcome {
        let Some(edit) = self.props.edit.clone() else {
            return Outcome::Refused {
                reason_key: "reason-not-editing",
            };
        };
        // The stored value's text committed unchanged: nothing to write (its display form need not parse back).
        if edit.initial.as_deref() == Some(edit.buffer.as_str()) {
            return self.act_props_cancel();
        }
        self.commit_text(&edit.row, &edit.buffer, None)
    }

    /// `props.set`: `row`, `value` (the text as typed), optional `choice` (an index for an ambiguous value).
    pub(super) fn act_props_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(Some(v)) => v.to_owned(),
            Ok(None) => return failed("error-missing-param", &["value"]),
        };
        let choice = match p.get("choice") {
            None => None,
            Some(v) => match v.as_u64().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => Some(i),
                None => return failed("error-bad-param", &["choice"]),
            },
        };
        let started = self.begin_edit(&key, RowAction::Edit, Some(value.clone()));
        if started != Outcome::Done {
            return started;
        }
        self.commit_text(&key, &value, choice)
    }

    /// Parses `text` for row `key` and commits it (a choice index picks an ambiguous reading).
    fn commit_text(&mut self, key: &str, text: &str, choice: Option<usize>) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed("error-no-row", &[key]);
        };
        if self
            .props
            .choices
            .as_ref()
            .is_some_and(|c| c.kind == ChoiceKind::Options)
        {
            self.props.choices = None;
        }
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let parsed = project.session.parse_field_input(&section.kind, &row.path, text);
        let parsed = match (parsed, choice) {
            (Ok(p), _) => p,
            (Err(InputError::Ambiguous { choices }), Some(i)) => match choices.get(i) {
                Some((_, v)) => ParsedInput {
                    value: v.clone(),
                    notes: Vec::new(),
                },
                None => return failed("error-bad-param", &["choice"]),
            },
            (Err(InputError::Ambiguous { choices }), None) => {
                self.props.choices = Some(Choices {
                    row: key.to_owned(),
                    items: choices,
                    kind: ChoiceKind::Ambiguous,
                    index: None,
                });
                return Outcome::Done;
            }
            (Err(InputError::Invalid(message)), _) => {
                self.props.error = Some((key.to_owned(), message.clone()));
                return Outcome::Failed { message };
            }
        };
        // A language object: the text replaces the source language's and the other languages stay (§6.5); an empty
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
        self.commit_value(key, value, text.to_owned(), parsed.notes)
    }

    /// Compiles the leaf `value` of row `key` into exactly one command and runs the commit path (§6.6).
    fn commit_value(&mut self, key: &str, value: Value, typed: String, notes: Vec<String>) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed("error-no-row", &[key]);
        };
        let Some(member) = member_path(&row.route) else {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
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
                    let message = strings::text("props-member-varies").to_owned();
                    self.props.error = Some((key.to_owned(), message.clone()));
                    return Outcome::Failed { message };
                }
                _ => None,
            }
        };
        let Some(new) = compose(&member, &row.path, base.as_ref(), &value) else {
            let message = strings::text("props-cannot-compose").to_owned();
            self.props.error = Some((key.to_owned(), message.clone()));
            return Outcome::Failed { message };
        };
        // A value equal to the current one builds no command (§6.6).
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
        if unchanged && !(is_derived(&row) && row.path == member) {
            self.props.edit = None;
            self.props.error = None;
            return Outcome::Done;
        }
        let edit = self.pending_edit(&section, &row, &member, value, new, base, typed);
        self.props.notes = notes;
        self.run_edit(edit, false)
    }

    /// The command of an edit.
    #[allow(clippy::too_many_arguments)]
    fn pending_edit(
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
            crate::properties::EditRoute::FieldsSet { kind, path } => (
                "fields.set".to_owned(),
                json!({ "kind": kind, "targets": targets, "path": path, "value": new }),
            ),
            crate::properties::EditRoute::ReadOnly(_) => (String::new(), Value::Null),
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

    /// The commit path of an edit (§6.6): the exposure guard, one preview, the policy, then the commit or the card.
    /// `card` forces the card (a rebuilt stale card).
    fn run_edit(&mut self, edit: PendingEdit, card: bool) -> Outcome {
        let row = edit.row.clone();
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let Some(meta) = project.session.command_meta(&edit.name) else {
            return failed("error-unknown-action", &[&edit.name]);
        };
        if meta.exposure != Exposure::Edit {
            return Outcome::Refused {
                reason_key: "reason-human-only",
            };
        }
        if !self.is_writable() {
            return Outcome::Refused {
                reason_key: "reason-read-only",
            };
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: edit.name.clone(),
            params: edit.params.clone(),
        };
        let submit = self.props_submit();
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
                let message = self.refusal(&e, section.as_ref());
                self.props.error = Some((row, message.clone()));
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
                    _ => "reason-rebuilt",
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
                Outcome::Done
            }
        }
    }

    /// The submission of the grid's commands: the GUI in the window, the script's command log headless.
    fn props_submit(&self) -> Submit {
        Submit::from(super::mode_origin(self.mode, &self.origin_file))
    }

    /// Commits a kept, fresh preview of the command `name` with `params`: the window `Session::commit`; headless a
    /// one-line command log through `Session::run_script` (`command.run`'s path: exposure, `--record`). A refusal
    /// names a target of `section` by its label (§6.7).
    pub(super) fn commit_kept(
        &mut self,
        state: PreviewState,
        name: &str,
        params: &Value,
        section: Option<&SheetSection>,
    ) -> Outcome {
        if self.mode == Mode::Headless {
            drop(state);
            return self.run_logged(name.to_owned(), params.clone());
        }
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let result = match state.result {
            Ok(preview) => {
                bump(calls, |c| c.commit += 1);
                project.session.commit(preview)
            }
            Err(message) => return Outcome::Failed { message },
        };
        let outcome = match result {
            Ok(_) => {
                self.record.undone.clear();
                self.record.steps.push(RecordedCommand {
                    cmd: name.to_owned(),
                    params: params.clone(),
                });
                Outcome::Done
            }
            Err(e) => Outcome::Failed {
                message: self.refusal(&e, section),
            },
        };
        self.refresh();
        outcome
    }

    /// Commits a kept, fresh preview of `edit` ([`ShellModel::commit_kept`]); a commit with several targets shows the
    /// toast.
    fn commit_edit(&mut self, state: PreviewState, edit: &PendingEdit) -> Outcome {
        let section = self.props_find(&edit.row).map(|(s, _)| s);
        let outcome = self.commit_kept(state, &edit.name, &edit.params, section.as_ref());
        match &outcome {
            Outcome::Done => {
                self.props.edit = None;
                self.props.error = None;
                self.props.choices = None;
                self.props.popup = None;
                if edit.targets.len() > 1 {
                    self.props.toasts += 1;
                    self.props.toast = Some(Toast {
                        text: strings::format(
                            "toast-set",
                            &[
                                &edit.label,
                                &edit.targets.len().to_string(),
                                &kind_plural(&edit.kind, edit.targets.len()),
                            ],
                        ),
                        undo: true,
                        serial: self.props.toasts,
                    });
                }
            }
            Outcome::Failed { message } => {
                self.props.error = Some((edit.row.clone(), message.clone()));
            }
            Outcome::Refused { .. } => {}
        }
        outcome
    }

    /// `props.choose`: `index` of the choices under a field (an ambiguous reading, the multi-language switch, a choice
    /// editor's options) or of the row's actions popup commits it; `step` (±1) moves the choices' highlight (Up,
    /// Down); with neither the highlighted choice is committed, else the buffer (Enter in a choice editor).
    pub(super) fn act_props_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return failed("error-bad-param", &["step"]);
            };
            let Some(c) = &mut self.props.choices else {
                return Outcome::Refused {
                    reason_key: "reason-no-choices",
                };
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
                None => return failed("error-bad-param", &["index"]),
            },
            None => match self.props.choices.as_ref().and_then(|c| c.index) {
                Some(i) => i,
                None if self.props.edit.is_some() => return self.act_props_commit(),
                None => return failed("error-missing-param", &["index"]),
            },
        };
        if let Some(Popup::Actions { row, .. }) = self.props.popup.clone() {
            let Some((_, r)) = self.props_find(&row) else {
                return failed("error-no-row", &[&row]);
            };
            let Some(action) = r.actions.get(index) else {
                return failed("error-bad-param", &["index"]);
            };
            self.props.popup = None;
            return self.row_action(&row, *action);
        }
        let Some(choices) = self.props.choices.take() else {
            return Outcome::Refused {
                reason_key: "reason-no-choices",
            };
        };
        let Some((_, value)) = choices.items.get(index).cloned() else {
            self.props.choices = Some(choices);
            return failed("error-bad-param", &["index"]);
        };
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

    /// `props.row_menu` (Shift+F10, the Menu key): the focused row's actions as a popup list; with the list open,
    /// `step` (±1) moves its highlight (Up, Down) and Enter is `props.choose` with the highlighted index.
    pub(super) fn act_props_row_menu(&mut self, p: &Map<String, Value>) -> Outcome {
        if let (Some(step), Some(Popup::Actions { row, index })) = (p.get("step"), self.props.popup.clone()) {
            let Some(step) = step.as_i64() else {
                return failed("error-bad-param", &["step"]);
            };
            let n = self.props_find(&row).map_or(1, |(_, r)| r.actions.len().max(1));
            let index = (index as i64 + step).clamp(0, n as i64 - 1) as usize;
            self.props.popup = Some(Popup::Actions { row, index });
            return Outcome::Done;
        }
        let Some(key) = self.properties_row() else {
            return Outcome::Refused {
                reason_key: "reason-no-row",
            };
        };
        let has_actions = self.props_find(&key).is_some_and(|(_, r)| !r.actions.is_empty());
        if !has_actions {
            return Outcome::Refused {
                reason_key: "reason-no-actions",
            };
        }
        self.props.popup = Some(Popup::Actions { row: key, index: 0 });
        Outcome::Done
    }

    /// `props.action`: `action` (`edit`, `clear`, `override`, `reset`, `freeze`, `unfreeze`) on `row` (else the
    /// focused row).
    pub(super) fn act_props_action(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let action = match string_param(p, "action") {
            Err(e) => return e,
            Ok(None) => return failed("error-missing-param", &["action"]),
            Ok(Some(word)) => match [
                RowAction::Edit,
                RowAction::Clear,
                RowAction::Override,
                RowAction::Reset,
                RowAction::Freeze,
                RowAction::Unfreeze,
            ]
            .into_iter()
            .find(|a| a.word() == word)
            {
                Some(a) => a,
                None => return failed("error-bad-param", &["action"]),
            },
        };
        self.row_action(&key, action)
    }

    /// A row action (§6.3, §6.4): its partner write, or the editor for Edit and Override.
    fn row_action(&mut self, key: &str, action: RowAction) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed("error-no-row", &[key]);
        };
        if let Some(reason_key) = self.props_refusal(&section, &row, action) {
            return Outcome::Refused { reason_key };
        }
        self.props.row = Some(key.to_owned());
        match action {
            RowAction::Edit => self.begin_edit(key, RowAction::Edit, None),
            RowAction::Override => self.begin_edit(key, RowAction::Override, None),
            RowAction::Clear | RowAction::Reset | RowAction::Unfreeze => {
                self.commit_value(key, Value::Null, String::new(), Vec::new())
            }
            // WP-S2.05 part 2 (§7.5): one `fields.set_many` entry per device, each with its own product designation.
            RowAction::Freeze if section.kind == "device" && section.targets.len() > 1 => {
                self.freeze_devices(&section, &row)
            }
            RowAction::Freeze => {
                let value = match (&row.value, section.kind.as_str()) {
                    (
                        RowValue::One {
                            json: Value::String(product),
                            ..
                        },
                        "device",
                    ) => {
                        json!({ "text": product, "sign": "shown" })
                    }
                    (RowValue::One { text, .. }, "connection") if text != "?" => json!({ "frozen": text }),
                    _ => {
                        let message = strings::text("props-cannot-freeze").to_owned();
                        self.props.error = Some((key.to_owned(), message.clone()));
                        return Outcome::Failed { message };
                    }
                };
                self.commit_value(key, value, String::new(), Vec::new())
            }
        }
    }

    // WP-S2.05 part 2
    /// Freeze on a device section with several targets (§7.5): every device whose tag is not frozen gets
    /// `tag.frozen` ← `{"text": <its product designation>, "sign": "shown"}`, all in one `fields.set_many` through the
    /// preview and the policy (rule 2 opens the card). Each device's product designation is the derived tag row of its
    /// own sheet: one `Session::property_sheet` call per device, on this action only (as "set all", ruling 7).
    fn freeze_devices(&mut self, section: &SheetSection, row: &PropRow) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let mut edits = Vec::new();
        for t in &section.targets {
            bump(&self.calls, |c| c.property_sheet += 1);
            let sheet = project.session.property_sheet(&[*t]);
            let Some((_, tag)) = find_row(&sheet, "device/derived.tag") else {
                continue;
            };
            match (&tag.value, &tag.provenance) {
                (_, Provenance::Frozen { .. }) => {}
                (
                    RowValue::One {
                        json: Value::String(product),
                        ..
                    },
                    _,
                ) if !product.is_empty() => {
                    edits.push(
                        json!({"kind": "device", "targets": [t.to_string()], "path": "tag.frozen",
                        "value": {"text": product, "sign": "shown"}}),
                    );
                }
                _ => {}
            }
        }
        if edits.is_empty() {
            let message = strings::text("props-cannot-freeze").to_owned();
            self.props.error = Some((row.key.clone(), message.clone()));
            return Outcome::Failed { message };
        }
        // The base of a stale card's check: the member as the sheet shows it now.
        let base = match self.props_find("device/tag.frozen").map(|(_, r)| r.value) {
            Some(RowValue::One { json, .. }) => Some(json),
            Some(RowValue::Varies { .. }) => Some(Value::String("‹varies›".into())),
            _ => None,
        };
        let edit = PendingEdit {
            row: row.key.clone(),
            kind: section.kind.clone(),
            // The section's targets, so that a stale card's check compares like with like.
            targets: section.targets.iter().map(ToString::to_string).collect(),
            route: row.route.clone(),
            path: "tag.frozen".to_owned(),
            leaf: None,
            base,
            name: "fields.set_many".to_owned(),
            params: json!({ "edits": edits }),
            typed: String::new(),
            label: crate::properties::row_label(section, row),
        };
        self.props.notes.clear();
        self.run_edit(edit, false)
    }

    /// `props.list`: `row` and `op` (`add` on a list row; `remove`, `up`, `down` on an item row).
    pub(super) fn act_props_list(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let op = match string_param(p, "op") {
            Err(e) => return e,
            Ok(Some(op)) => op.to_owned(),
            Ok(None) => return failed("error-missing-param", &["op"]),
        };
        let Some((section, row)) = self.props_find(&key) else {
            return failed("error-no-row", &[&key]);
        };
        // The list row and the item's index.
        let (list_key, index) = match row.key.rsplit_once('[') {
            Some((head, tail)) if tail.ends_with(']') && op != "add" => {
                match tail.trim_end_matches(']').parse::<usize>() {
                    Ok(i) => (head.to_owned(), Some(i)),
                    Err(_) => return failed("error-bad-param", &["row"]),
                }
            }
            _ => (row.key.clone(), None),
        };
        let Some((_, list)) = self.props_find(&list_key) else {
            return failed("error-no-row", &[&list_key]);
        };
        if let Some(reason_key) = self.props_refusal(&section, &list, RowAction::Edit) {
            return Outcome::Refused { reason_key };
        }
        let crate::properties::FieldType::List(item) = list.ty.inner() else {
            return failed("error-bad-param", &["row"]);
        };
        let mut items = match &list.value {
            RowValue::One {
                json: Value::Array(a),
                ..
            } => a.clone(),
            RowValue::Absent => Vec::new(),
            _ => {
                let message = strings::text("props-member-varies").to_owned();
                return Outcome::Failed { message };
            }
        };
        match (op.as_str(), index) {
            ("add", _) => items.push(item_default(item)),
            ("remove", Some(i)) if i < items.len() => {
                items.remove(i);
            }
            ("up", Some(i)) if i > 0 && i < items.len() => items.swap(i - 1, i),
            ("down", Some(i)) if i + 1 < items.len() => items.swap(i, i + 1),
            _ => return failed("error-bad-param", &["op"]),
        }
        self.commit_value(&list_key, Value::Array(items), String::new(), Vec::new())
    }

    /// `props.mltext`: without `text` it opens the multi-language popup of the focused row; with it, `text` becomes
    /// the neutral text (no `lang`) or the text of `lang`. Switching between the forms asks first (FR-I18N-001).
    pub(super) fn act_props_mltext(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some((section, row)) = self.props_find(&key) else {
            return failed("error-no-row", &[&key]);
        };
        if !matches!(row.ty.inner(), crate::properties::FieldType::MlText) {
            return Outcome::Refused {
                reason_key: "reason-not-mltext",
            };
        }
        if let Some(reason_key) = self.props_refusal(&section, &row, RowAction::Edit)
            && !(is_derived(&row) && row.actions.contains(&RowAction::Override))
        {
            return Outcome::Refused { reason_key };
        }
        let lang = match string_param(p, "lang") {
            Err(e) => return e,
            Ok(l) => l.map(str::to_owned),
        };
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(None) => {
                self.props.row = Some(key.clone());
                self.props.popup = Some(Popup::MlText { row: key });
                return Outcome::Done;
            }
            Ok(Some(t)) => t.to_owned(),
        };
        let current = match &row.value {
            RowValue::One { json, .. } if !is_derived(&row) => json.clone(),
            _ => Value::Null,
        };
        let source = self
            .props
            .sheet()
            .and_then(|s| s.languages.first().cloned())
            .unwrap_or_else(|| "en".to_owned());
        let choice = |label: &str, value: Value| (strings::text(label).to_owned(), value);
        let (direct, choices): (Option<Value>, Vec<(String, Value)>) = match (&lang, &current) {
            (None, Value::Object(_)) => (
                None,
                vec![
                    choice("mltext-replace", Value::String(text.clone())),
                    choice("mltext-keep", {
                        let mut m = current.as_object().cloned().unwrap_or_default();
                        m.insert(source.clone(), Value::String(text.clone()));
                        Value::Object(m)
                    }),
                ],
            ),
            (None, _) => (
                Some(if text.is_empty() {
                    Value::Null
                } else {
                    Value::String(text.clone())
                }),
                Vec::new(),
            ),
            (Some(l), Value::String(neutral)) if !neutral.is_empty() => (
                None,
                vec![
                    choice("mltext-replace", json!({ l.clone(): text })),
                    choice("mltext-keep", {
                        let mut m = Map::new();
                        m.insert(source.clone(), Value::String(neutral.clone()));
                        m.insert(l.clone(), Value::String(text.clone()));
                        Value::Object(m)
                    }),
                ],
            ),
            (Some(l), Value::Object(m)) => {
                let mut m = m.clone();
                if text.is_empty() {
                    m.remove(l);
                } else {
                    m.insert(l.clone(), Value::String(text.clone()));
                }
                (
                    Some(if m.is_empty() {
                        Value::Null
                    } else {
                        Value::Object(m)
                    }),
                    Vec::new(),
                )
            }
            (Some(l), _) => (Some(json!({ l.clone(): text })), Vec::new()),
        };
        self.props.popup = None;
        match direct {
            Some(value) => self.commit_value(&key, value, text, Vec::new()),
            None => {
                self.props.choices = Some(Choices {
                    row: key,
                    items: choices,
                    kind: ChoiceKind::MlTextSwitch,
                    index: None,
                });
                Outcome::Done
            }
        }
    }

    /// `preview.apply` (the card's Apply, Ctrl+Enter): a fresh card commits; a stale one is checked and rebuilt.
    pub(super) fn act_preview_apply(&mut self) -> Outcome {
        let Some(card) = self.props.card.take() else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        // WP-S2.05 part 2: a replace card is the find pane's (§7.4).
        if matches!(card.origin, CardOrigin::Find { .. }) {
            return self.apply_find_card(card);
        }
        // FT-410: an import card is the parts table's (`parts.import`).
        if matches!(card.origin, CardOrigin::Import { .. }) {
            return self.apply_import_card(card);
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            let message = message.clone();
            self.props.error = Some((card.edit.row.clone(), message.clone()));
            return Outcome::Failed { message };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            return self.commit_edit(preview, &edit);
        }
        // Stale (§6.6): never re-preview the old parameters.
        let edit = card.edit;
        let member = member_path(&edit.route).unwrap_or_default();
        let member_key = format!("{}/{member}", edit.kind);
        let now = match self.props_find(&member_key).map(|(_, r)| r.value) {
            Some(RowValue::One { json, .. }) => Some(json),
            Some(RowValue::Absent) | None => None,
            Some(RowValue::Varies { .. }) => Some(Value::String("‹varies›".into())),
        };
        let same_targets = self.props_find(&edit.row).is_some_and(|(s, _)| {
            s.targets.iter().map(ToString::to_string).collect::<Vec<_>>() == edit.targets
        });
        if now != edit.base || !same_targets {
            let message = strings::text("props-changed-meanwhile").to_owned();
            self.props.error = Some((edit.row.clone(), message));
            self.props.edit = Some(EditBuffer {
                row: edit.row.clone(),
                buffer: edit.typed.clone(),
                initial: None,
            });
            return Outcome::Refused {
                reason_key: "reason-changed-meanwhile",
            };
        }
        // WP-S2.05 part 2: a freeze on several devices is built again from the current sheet (a second Apply).
        if edit.name == "fields.set_many" {
            return self.row_action(&edit.row, RowAction::Freeze);
        }
        let Some((section, row)) = self.props_find(&edit.row) else {
            return failed("error-no-row", &[&edit.row]);
        };
        let new = match &edit.leaf {
            Some(leaf) => compose(&member, &edit.path, now.as_ref(), leaf),
            None => edit.params.get("value").cloned(),
        };
        let Some(new) = new else {
            return failed("error-no-row", &[&edit.row]);
        };
        let leaf = edit.leaf.clone().unwrap_or_else(|| new.clone());
        let rebuilt = self.pending_edit(&section, &row, &member, leaf, new, now, edit.typed.clone());
        self.run_edit(rebuilt, true)
    }

    /// Shows `text` as the toast (FT-485): what a command did that the page does not show by itself (a new strip, an
    /// export, the wires a placement drew). It stays until it is dismissed or the next command replaces it; `undo`
    /// offers Undo.
    pub(super) fn notify(&mut self, text: String, undo: bool) {
        self.props.toasts += 1;
        self.props.toast = Some(Toast {
            text,
            undo,
            serial: self.props.toasts,
        });
    }

    /// `preview.cancel` (the card's Cancel, Escape): the preview is dropped.
    pub(super) fn act_preview_cancel(&mut self) -> Outcome {
        if self.props.card.take().is_none() {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        }
        Outcome::Done
    }

    /// `toast.undo`: the registry's undo; the toast goes.
    pub(super) fn act_toast_undo(&mut self) -> Outcome {
        if self.props.toast.take().is_none() {
            return Outcome::Refused {
                reason_key: "reason-no-toast",
            };
        }
        self.dispatch("edit.undo", &Value::Null)
    }

    /// `toast.dismiss`: the toast goes.
    pub(super) fn act_toast_dismiss(&mut self) -> Outcome {
        self.props.toast = None;
        Outcome::Done
    }

    /// The grid's part of the Escape chain (§6.9) while it has the focus: the choices or a popup, then the edit, then
    /// the card, then the focus goes back to the canvas. `None` when the grid does not have the focus.
    pub(super) fn dismiss_properties(&mut self) -> Option<Outcome> {
        if self.focus != Some(Focus::Properties) {
            return None;
        }
        if self.props.choices.take().is_some() || self.props.popup.take().is_some() {
            return Some(Outcome::Done);
        }
        if self.props.edit.is_some() || self.props.error.is_some() {
            return Some(self.act_props_cancel());
        }
        if self.props.card.take().is_some() {
            return Some(Outcome::Done);
        }
        self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
        Some(Outcome::Done)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use ew_api::Origin;
    use serde_json::json;

    use super::*;
    use crate::properties::EditRoute;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    /// D4: the grid never builds a command whose exposure is not `Edit`. A pending edit naming the `HumanOnly`
    /// `project.checks.set` (no row routes there, so only a forged edit can) is refused before any preview.
    #[test]
    fn a_human_only_command_is_refused_before_its_preview() {
        let mut shell = ShellModel::for_tests();
        let dir = std::env::temp_dir().join(format!("ew-d4-{}", std::process::id()));
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        copy_dir(&golden, &dir);
        assert_eq!(
            shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
            Outcome::Done
        );
        let before = shell.session_calls();
        let edit = PendingEdit {
            row: "project/settings.checks".into(),
            kind: "project".into(),
            targets: Vec::new(),
            route: EditRoute::SettingsMember {
                member: "checks".into(),
            },
            path: "settings.checks".into(),
            leaf: None,
            base: None,
            name: "project.checks.set".into(),
            params: json!({"release_scheme": "pre-delivery"}),
            typed: String::new(),
            label: "Checks".into(),
        };
        assert_eq!(
            shell.run_edit(edit, false),
            Outcome::Refused {
                reason_key: "reason-human-only"
            }
        );
        let after = shell.session_calls();
        assert_eq!(after.preview_with, before.preview_with, "no preview");
        assert_eq!(after.commit, before.commit);
        assert_eq!(after.run_script, before.run_script);
        // §6.7: a refusal at `/targets/<i>` names that target by its label, also among several targets.
        assert_eq!(
            shell.dispatch("select.set", &json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            Outcome::Done
        );
        let section = shell.props.sheet().unwrap().sections[0].clone();
        let err = CmdError::invalid_params("fields.set", "/targets/1", "refused");
        let labels: Vec<String> = section
            .targets
            .iter()
            .map(|t| {
                shell
                    .project
                    .as_ref()
                    .unwrap()
                    .session
                    .property_sheet(&[*t])
                    .header
                    .title
            })
            .collect();
        assert_eq!(
            shell.refusal(&err, Some(&section)),
            format!("on {}: refused", labels[1])
        );
        assert!(labels.iter().all(|l| l.starts_with('-')), "{labels:?}");
        assert_eq!(
            refusal_text(&CmdError::invalid_params("fields.set", "/value", "bad"), &|_| {
                String::new()
            }),
            "bad"
        );
        shell.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FT-404: the shell's undo and redo carry its origin. A `project.checks.set` step committed from the GUI is
    /// refused to the headless script (`Origin::CmdLog`) with the history reason key, the session unchanged; the
    /// window's GUI origin undoes and redoes it.
    #[test]
    fn a_human_only_step_is_undone_only_in_the_window() {
        let mut shell = ShellModel::for_tests();
        let dir = std::env::temp_dir().join(format!("ew-ft404-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        copy_dir(&golden, &dir);
        assert_eq!(
            shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
            Outcome::Done
        );
        assert_eq!(shell.mode, Mode::Headless);
        let cmd = AnyCommand {
            name: "project.checks.set".into(),
            params: json!({"release_scheme": "full"}),
        };
        let session = &mut shell.project.as_mut().unwrap().session;
        session
            .execute_with(&cmd, Submit::from(Origin::Gui))
            .expect("the GUI sets the release scheme");
        let cursor = |shell: &ShellModel| shell.project.as_ref().unwrap().session.history().cursor();
        let rev = |shell: &ShellModel| shell.project.as_ref().unwrap().session.world().rev;
        let (applied, at) = (cursor(&shell), rev(&shell));
        let refused = Outcome::Refused {
            reason_key: "reason-human-only-history",
        };
        assert_eq!(shell.dispatch("edit.undo", &Value::Null), refused);
        assert_eq!(
            (cursor(&shell), rev(&shell)),
            (applied, at),
            "a refused undo changes nothing"
        );
        assert!(
            strings::text("reason-human-only-history").contains("ADR-0008 amendment 3"),
            "the reason names the rule"
        );
        shell.mode = Mode::Gui;
        assert_eq!(shell.dispatch("edit.undo", &Value::Null), Outcome::Done);
        assert_eq!(cursor(&shell), applied - 1);
        shell.mode = Mode::Headless;
        let at = rev(&shell);
        assert_eq!(shell.dispatch("edit.redo", &Value::Null), refused);
        assert_eq!(
            (cursor(&shell), rev(&shell)),
            (applied - 1, at),
            "a refused redo changes nothing"
        );
        shell.mode = Mode::Gui;
        assert_eq!(shell.dispatch("edit.redo", &Value::Null), Outcome::Done);
        assert_eq!(cursor(&shell), applied);
        shell.shutdown();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
