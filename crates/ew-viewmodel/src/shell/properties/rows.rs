// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Properties pane's rows beyond their values: a row's menu and its actions, the unused connection points, the list and multi-language editors, and the card's Apply and Cancel.

use crate::key;
use crate::properties::{
    Card, CardOrigin, ChoiceKind, Choices, EditBuffer, PendingEdit, Popup, PropRow, Provenance, RowAction,
    RowMenuItem, RowValue, SheetSection, find_row, is_cp_row, is_derived, item_default, member_path,
};
use crate::shell::navigation::{Focus, bump};
use crate::shell::{Outcome, ShellModel, failed, string_param};
use crate::strings;
use crate::strings::Key;
use ew_api::AnyCommand;
use ew_api::output::table_edit::compose;
use serde_json::{Map, Value, json};

impl ShellModel {
    /// The entries of the actions menu of the row `key`: the row's actions, then Make variable
    /// when the row can be a macro variable (a row of one record with an action, not a derived value), then on a
    /// Parts row Choose part…, Type not known yet…, Add part… and Edit part…; then Go to source on a row whose value has a source to go to,
    /// Define potential… on the Potential row of a potential no definition names, and Mark not used or Mark used on a
    /// device's connection point; empty for a row that is not on show.
    pub fn row_menu(&self, key: &str) -> Vec<RowMenuItem> {
        // The window asks this of every row it draws: the row is borrowed, not cloned.
        let Some((section, row)) = self.props.sheet().and_then(|sheet| find_row(sheet, key)) else {
            return Vec::new();
        };
        let mut out: Vec<RowMenuItem> = row.actions.iter().map(|a| RowMenuItem::Action(*a)).collect();
        if !out.is_empty() && section.targets.len() == 1 && !row.path.starts_with("derived.") {
            out.push(RowMenuItem::MakeVariable);
        }
        // An editable Parts row (a part-uses row) adds Choose part… (the first use when the
        // row names a part), Add part… and, when it names a part, Edit part….
        if row.ty.is_part_uses() && row.actions.contains(&RowAction::Edit) {
            let has_part = matches!(&row.value, RowValue::One { json, .. } if json.as_array().is_some_and(|a| !a.is_empty()));
            out.push(RowMenuItem::ChoosePart { first_use: has_part });
            out.push(RowMenuItem::Placeholder);
            out.push(RowMenuItem::AddPart);
            if has_part {
                out.push(RowMenuItem::EditPart);
            }
        }
        // A derived or inherited value's source.
        if row.source.is_some() {
            out.push(RowMenuItem::GoToSource);
        }
        let writable = self.is_writable() && self.props.sheet().is_some_and(|s| s.read_only.is_none());
        // The Potential row of a potential no definition names: Define potential… there.
        if writable
            && row.key.ends_with("/derived.potential")
            && row.source.is_none()
            && section.targets.len() == 1
        {
            out.push(RowMenuItem::DefinePotential);
        }
        // A device's connection point: Mark not used, or Mark used.
        if let (true, RowValue::One { json, .. }) = (writable && is_cp_row(row), &row.value)
            && let Some(unused) = json.get("unused").and_then(Value::as_bool)
        {
            out.push(RowMenuItem::CpUnused { on: !unused });
        }
        // A device's Function text: Function text over its path, which prints it on the page, while one of its
        // symbols is selected (the symbol's right-click menu has it too).
        if row.key == "device/function_text"
            && crate::actions::find("text.function_over_path").is_some_and(|m| m.enabled_now(self).is_ok())
        {
            out.push(RowMenuItem::FunctionOverPath);
        }
        // A field of the project's own: Rename… and Remove.
        if writable && crate::shell::own_fields::own_key(&row.key).is_some() {
            out.push(RowMenuItem::RenameField);
            out.push(RowMenuItem::RemoveField);
        }
        out
    }

    /// The tag of the cable whose core gives the value of the derived row `key` (a wire's cross-section or colour), which
    /// is changed at the cable; `None` for any other row, among them a row its Override sets (a size from the rating
    /// of a protecting device names that device's Parts row too).
    pub fn cable_row(&self, key: &str) -> Option<String> {
        let (_, row) = self.props.sheet().and_then(|sheet| find_row(sheet, key))?;
        let Some(ew_api::output::SourceLink::Record { id, row: to }) = &row.source else {
            return None;
        };
        if to != ew_api::output::CABLE_SOURCE_ROW
            || !is_derived(row)
            || row.actions.contains(&RowAction::Override)
        {
            return None;
        }
        self.project.as_ref()?.session.device_tag(*id)
    }

    /// Go to source: the row that holds the source of row `key`'s value, with the cursor on it; the sheet
    /// shows the record or the project first when it does not show the row, and the row's category unfolds.
    pub(in crate::shell) fn props_go_to_source(&mut self, key: &str) -> Outcome {
        let Some((_, row)) = self.props_find(key) else {
            return failed(key!("error-no-row"), &[key]);
        };
        let Some(link) = row.source else {
            return Outcome::refused(key!("reason-action-not-offered"));
        };
        let (target, to) = match link {
            ew_api::output::SourceLink::Record { id, row } => (Some(id), row),
            ew_api::output::SourceLink::Project { row } => (None, row),
        };
        let shows = |s: &Self| {
            s.props.sheet().is_some_and(|sheet| {
                sheet.sections.iter().any(|sec| {
                    target.is_none_or(|t| sec.targets.contains(&t)) && sec.rows.iter().any(|r| r.key == to)
                })
            })
        };
        if !shows(self) {
            let out = match target {
                Some(id) => self.act_select_set(
                    &json!({ "ids": [id.to_string()] })
                        .as_object()
                        .cloned()
                        .unwrap_or_default(),
                ),
                None => self.act_props_project(),
            };
            if out != Outcome::Done {
                return out;
            }
            self.sync_properties();
        }
        let Some(sheet) = self.props.sheet.clone() else {
            return Outcome::refused(key!("reason-no-sheet"));
        };
        let Some((section, found)) = find_row(&sheet, &to) else {
            return failed(key!("error-no-row"), &[&to]);
        };
        let group = crate::properties::group_key(&section.kind, crate::properties::group_of(section, found));
        if self.set_group_open(group, true) {
            self.save_settings();
        }
        self.props.row = Some(to);
        self.props.popup = None;
        self.focus = Some(Focus::Properties);
        Outcome::Done
    }

    /// The point on the page on show (page nm) of the connection point whose row of a device's Properties
    /// has the cursor (a row the person chose, not the first row on show), which the canvas rings in the selection
    /// colour: which "+" of a power supply a row names is then answered by looking. `None` for any other row, or when
    /// the point is drawn on another page.
    pub fn properties_cp_point(&self) -> Option<(i64, i64)> {
        let key = self.props.moved_row()?;
        if self.properties_row().as_deref() != Some(key) {
            return None;
        }
        let row = self
            .props
            .sheet()?
            .sections
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.key == key && is_cp_row(r))?;
        let RowValue::One { json, .. } = &row.value else {
            return None;
        };
        let function = json.get("function")?.as_str()?;
        let idx = json.get("idx")?.as_u64()?;
        let canvas = self.canvas()?;
        canvas
            .page()
            .tables
            .cps
            .iter()
            .find(|c| u64::from(c.idx) == idx && c.function.to_string() == function)
            .map(|c| c.at)
    }

    /// Whether the connection point `idx` of the function `function` is marked not used (its override's `unused`).
    pub fn cp_unused(&self, function: ew_api::output::Uuid, idx: u16) -> bool {
        let Some(project) = &self.project else {
            return false;
        };
        match project.session.stored_field("function", function, "cp_overrides") {
            Some(Value::Array(overrides)) => overrides.iter().any(|o| {
                o.get("idx").and_then(Value::as_u64) == Some(u64::from(idx))
                    && o.get("unused") == Some(&Value::Bool(true))
            }),
            _ => false,
        }
    }

    /// `cp.unused {function, idx, on?}` (Mark not used, Mark used): marks the connection point `idx` of
    /// the function `function` as not used by design (`on`, else the other state), or as used again, in its
    /// override (`cp_overrides[].unused`), as one `fields.set` and one undo step. Its unconnected-point finding
    /// (EW-POT-009) then drops to an info.
    pub(crate) fn act_cp_unused(&mut self, p: &Map<String, Value>) -> Outcome {
        let project = or_return!(crate::shell::gated(&self.project));
        let function = match string_param(p, "function") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["function"]),
            Ok(Some(f)) => f.to_owned(),
        };
        let Some(id) = ew_api::output::Uuid::parse_str(&function)
            .ok()
            .filter(|id| project.session.stored_field("function", *id, "id").is_some())
        else {
            return failed(key!("error-bad-param"), &["function"]);
        };
        let Some(idx) = p
            .get("idx")
            .and_then(Value::as_u64)
            .and_then(|i| u16::try_from(i).ok())
        else {
            return failed(key!("error-bad-param"), &["idx"]);
        };
        let mut overrides = match project.session.stored_field("function", id, "cp_overrides") {
            Some(Value::Array(a)) => a,
            _ => Vec::new(),
        };
        let at = overrides
            .iter()
            .position(|o| o.get("idx").and_then(Value::as_u64) == Some(u64::from(idx)));
        let now = at
            .and_then(|i| overrides[i].get("unused"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let on = match p.get("on") {
            None => !now,
            Some(v) => match v.as_bool() {
                Some(b) => b,
                None => return failed(key!("error-bad-param"), &["on"]),
            },
        };
        if on == now {
            return Outcome::Done;
        }
        match at {
            Some(i) if on => overrides[i]["unused"] = Value::Bool(true),
            // Used again: the override goes when nothing else is left in it.
            Some(i) => {
                if let Some(o) = overrides[i].as_object_mut() {
                    o.remove("unused");
                    if o.len() == 1 {
                        overrides.remove(i);
                    }
                }
            }
            None => {
                overrides.push(json!({ "idx": idx, "unused": true }));
                overrides.sort_by_key(|o| o.get("idx").and_then(Value::as_u64));
            }
        }
        let value = if overrides.is_empty() {
            Value::Null
        } else {
            Value::Array(overrides)
        };
        let cmd = AnyCommand {
            name: "fields.set".into(),
            params: json!({
                "kind": "function",
                "targets": [function],
                "path": "cp_overrides",
                "value": value,
            }),
        };
        self.commit_from_host(&cmd)
    }

    /// `props.row_menu` (Shift+F10, the Menu key): the focused row's actions as a popup list; with the list open,
    /// `step` (±1) moves its highlight (Up, Down) and Enter is `props.choose` with the highlighted index.
    pub(crate) fn act_props_row_menu(&mut self, p: &Map<String, Value>) -> Outcome {
        if let (Some(step), Some(Popup::Actions { row, index })) = (p.get("step"), self.props.popup.clone()) {
            let Some(step) = step.as_i64() else {
                return failed(key!("error-bad-param"), &["step"]);
            };
            let n = self.row_menu(&row).len().max(1);
            let index = (index as i64 + step).clamp(0, n as i64 - 1) as usize;
            self.props.popup = Some(Popup::Actions { row, index });
            return Outcome::Done;
        }
        let Some(key) = self.properties_row() else {
            return Outcome::refused(key!("reason-no-row"));
        };
        let menu = self.row_menu(&key);
        if menu.is_empty() {
            return Outcome::refused(key!("reason-no-actions"));
        }
        // A row whose only action is Edit opens its editor at once, not a menu of one entry.
        if menu == [RowMenuItem::Action(RowAction::Edit)] {
            return self.row_action(&key, RowAction::Edit);
        }
        self.props.popup = Some(Popup::Actions { row: key, index: 0 });
        Outcome::Done
    }

    /// `props.action`: `action` (`edit`, `clear`, `override`, `reset`, `freeze`, `unfreeze`) on `row` (else the
    /// focused row).
    pub(crate) fn act_props_action(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(self.props_row_param(p));
        let action = match string_param(p, "action") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["action"]),
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
                None => return failed(key!("error-bad-param"), &["action"]),
            },
        };
        self.row_action(&key, action)
    }

    /// A row action: its partner write, or the editor for Edit and Override.
    pub(super) fn row_action(&mut self, key: &str, action: RowAction) -> Outcome {
        let Some((section, row)) = self.props_find(key) else {
            return failed(key!("error-no-row"), &[key]);
        };
        if let Some(out) = self.props_refusal(&section, &row, action) {
            // Tips 2 and 3.
            self.tip_for_refused_row(&section, &row, &out);
            return out;
        }
        self.props.row = Some(key.to_owned());
        match action {
            // The row New field of your own… opens its form.
            RowAction::Edit if key == crate::shell::own_fields::OWN_NEW_ROW => {
                self.act_field_new(&Map::new())
            }
            // The frame's Columns and rows: a click chooses the rulers.
            RowAction::Edit if key == crate::shell::frame_edit::RULERS_ROW => {
                self.frame_choose(crate::tools::frame::FrameChoice::Rulers)
            }
            RowAction::Edit if key == crate::shell::frame_edit::OWN_FRAME_ROW => self.act_frame_own_page(),
            RowAction::Edit if key == crate::shell::frame_edit::ADOPT_ROW => {
                self.act_frame_adopt(&Map::new())
            }
            RowAction::Edit => self.begin_edit(key, RowAction::Edit, None),
            RowAction::Override => self.begin_edit(key, RowAction::Override, None),
            RowAction::Clear | RowAction::Reset | RowAction::Unfreeze => {
                self.commit_value(key, Value::Null, String::new(), Vec::new())
            }
            // One `fields.set_many` entry per device, each with its own product designation.
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
                        let message = strings::text(key!("props-cannot-freeze")).to_owned();
                        self.error_under_row(key.to_owned(), message.clone());
                        return Outcome::Failed { message };
                    }
                };
                self.commit_value(key, value, String::new(), Vec::new())
            }
        }
    }

    /// Freeze on a device section with several targets: every device whose tag is not frozen gets
    /// `tag.frozen` ← `{"text": <its product designation>, "sign": "shown"}`, all in one `fields.set_many` through the
    /// preview and the policy (rule 2 opens the card). Each device's product designation is the derived tag row of its
    /// own sheet: one `Session::property_sheet` call per device, on this action only (as "set all").
    pub(super) fn freeze_devices(&mut self, section: &SheetSection, row: &PropRow) -> Outcome {
        let project = or_return!(crate::shell::gated(&self.project));
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
            let message = strings::text(key!("props-cannot-freeze")).to_owned();
            self.error_under_row(row.key.clone(), message.clone());
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
    pub(crate) fn act_props_list(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(self.props_row_param(p));
        let op = match string_param(p, "op") {
            Err(e) => return e,
            Ok(Some(op)) => op.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["op"]),
        };
        let Some((section, row)) = self.props_find(&key) else {
            return failed(key!("error-no-row"), &[&key]);
        };
        // The list row and the item's index.
        let (list_key, index) = match row.key.rsplit_once('[') {
            Some((head, tail)) if tail.ends_with(']') && op != "add" => {
                match tail.trim_end_matches(']').parse::<usize>() {
                    Ok(i) => (head.to_owned(), Some(i)),
                    Err(_) => return failed(key!("error-bad-param"), &["row"]),
                }
            }
            _ => (row.key.clone(), None),
        };
        let Some((_, list)) = self.props_find(&list_key) else {
            return failed(key!("error-no-row"), &[&list_key]);
        };
        if let Some(out) = self.props_refusal(&section, &list, RowAction::Edit) {
            return out;
        }
        let crate::properties::FieldType::List(item) = list.ty.inner() else {
            return failed(key!("error-bad-param"), &["row"]);
        };
        let mut items = match &list.value {
            RowValue::One {
                json: Value::Array(a),
                ..
            } => a.clone(),
            RowValue::Absent => Vec::new(),
            _ => {
                let message = strings::text(key!("props-member-varies")).to_owned();
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
            _ => return failed(key!("error-bad-param"), &["op"]),
        }
        self.commit_value(&list_key, Value::Array(items), String::new(), Vec::new())
    }

    /// `props.mltext`: without `text` it opens the multi-language popup of the focused row; with it, `text` becomes
    /// the neutral text (no `lang`) or the text of `lang`. Switching between the forms asks first (FR-I18N-001).
    pub(crate) fn act_props_mltext(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = or_return!(self.props_row_param(p));
        let Some((section, row)) = self.props_find(&key) else {
            return failed(key!("error-no-row"), &[&key]);
        };
        if !matches!(row.ty.inner(), crate::properties::FieldType::MlText) {
            return Outcome::refused(key!("reason-not-mltext"));
        }
        if let Some(out) = self.props_refusal(&section, &row, RowAction::Edit)
            && !(is_derived(&row) && row.actions.contains(&RowAction::Override))
        {
            return out;
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
        let choice = |label: Key<'_>, value: Value| (strings::text(label).to_owned(), value);
        let (direct, choices): (Option<Value>, Vec<(String, Value)>) = match (&lang, &current) {
            (None, Value::Object(_)) => (
                None,
                vec![
                    choice(key!("mltext-replace"), Value::String(text.clone())),
                    choice(key!("mltext-keep"), {
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
                    choice(key!("mltext-replace"), json!({ l.clone(): text })),
                    choice(key!("mltext-keep"), {
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
                    details: Vec::new(),
                    kind: ChoiceKind::MlTextSwitch,
                    index: None,
                });
                Outcome::Done
            }
        }
    }

    /// `preview.apply` (the card's Apply, Ctrl+Enter): a fresh card commits; a stale one is checked and rebuilt.
    pub(crate) fn act_preview_apply(&mut self) -> Outcome {
        let Some(card) = self.props.card.take() else {
            return Outcome::refused(key!("reason-no-card"));
        };
        // A table import card commits the import's own command.
        if matches!(card.origin, CardOrigin::TableImport(_)) {
            return self.apply_table_import_card(card);
        }
        // A replace card is the find pane's.
        if matches!(card.origin, CardOrigin::Find { .. }) {
            return self.apply_find_card(card);
        }
        // An import card is the parts table's (`parts.import`).
        if matches!(card.origin, CardOrigin::Import { .. }) {
            return self.apply_import_card(card);
        }
        // A reorder card is the Pages navigator's (`pages.reorder`).
        if matches!(card.origin, CardOrigin::Reorder { .. }) {
            return self.apply_reorder_card(card);
        }
        // A quick-fix card is the Messages pane's (`messages.fix`).
        if matches!(card.origin, CardOrigin::QuickFix { .. }) {
            return self.apply_fix_card(card);
        }
        // The paste choice card previews nothing; its choices are `paste.resolve`.
        if matches!(card.origin, CardOrigin::PasteVersions(_)) {
            self.props.card = Some(card);
            return Outcome::refused(key!("reason-paste-versions"));
        }
        // Update the project's copy commits, then the pending paste is armed again.
        if matches!(card.origin, CardOrigin::PasteUpdate { .. }) {
            return self.apply_paste_update_card(card);
        }
        // A cabinet's Route card stores its ways and says how many.
        if matches!(card.origin, CardOrigin::Cabinet(_)) {
            return self.apply_cabinet_card(card);
        }
        // A route card commits its plan and says what it routed.
        if matches!(card.origin, CardOrigin::Route { .. }) {
            return self.apply_route_card(card);
        }
        // An Arrange and route card commits its plan and says what moved and what it routed.
        if matches!(card.origin, CardOrigin::Arrange(_)) {
            return self.apply_arrange_card(card);
        }
        // A bulk card is a renumber dialog's, and Copy pages…'s.
        if matches!(
            card.origin,
            CardOrigin::Bulk { .. } | CardOrigin::Part(_) | CardOrigin::PageCopy { .. }
        ) {
            return self.apply_bulk_card(card);
        }
        // A macro card is the Macros navigator's (`macro.insert`).
        if matches!(card.origin, CardOrigin::Macro(_)) {
            return self.apply_macro_card(card);
        }
        // A table card is a navigator's table gesture.
        if matches!(card.origin, CardOrigin::Table { .. }) {
            return self.apply_table_card(card);
        }
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            let message = message.clone();
            self.error_under_row(card.edit.row.clone(), message.clone());
            return Outcome::Failed { message };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            return self.commit_edit(preview, &edit);
        }
        // Stale: never re-preview the old parameters.
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
            let message = strings::text(key!("props-changed-meanwhile")).to_owned();
            self.error_under_row(edit.row.clone(), message);
            self.props.edit = Some(EditBuffer {
                row: edit.row.clone(),
                buffer: edit.typed.clone(),
                initial: None,
            });
            return Outcome::refused(key!("reason-changed-meanwhile"));
        }
        // A freeze on several devices is built again from the current sheet (a second Apply).
        if edit.name == "fields.set_many" {
            return self.row_action(&edit.row, RowAction::Freeze);
        }
        let Some((section, row)) = self.props_find(&edit.row) else {
            return failed(key!("error-no-row"), &[&edit.row]);
        };
        let new = match &edit.leaf {
            Some(leaf) => compose(&member, &edit.path, now.as_ref(), leaf),
            None => edit.params.get("value").cloned(),
        };
        let Some(new) = new else {
            return failed(key!("error-no-row"), &[&edit.row]);
        };
        let leaf = edit.leaf.clone().unwrap_or_else(|| new.clone());
        let rebuilt = self.pending_edit(&section, &row, &member, leaf, new, now, edit.typed.clone());
        self.run_edit(rebuilt, true)
    }

    /// `preview.cancel` (the card's Cancel, Escape): the preview is dropped.
    pub(crate) fn act_preview_cancel(&mut self) -> Outcome {
        if self.props.card.take().is_none() {
            return Outcome::refused(key!("reason-no-card"));
        }
        Outcome::Done
    }

    /// The grid's part of the Escape chain while it has the focus: the choices or a popup, then the edit, then
    /// the card, then the focus goes back to the canvas. `None` when the grid does not have the focus.
    pub(in crate::shell) fn dismiss_properties(&mut self) -> Option<Outcome> {
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
