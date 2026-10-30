// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11): the host duties of the Macros and Libraries navigators, Save as macro… and Make variable.
//!
//! **Insert.** Enter on a macro row of the Macros navigator (or Draw ▸ Insert macro… with one under the cursor, or a
//! drop of the row) arms the macro tool ([`crate::tools::MacroTool`]); its click asks for the host step `macro.place`,
//! which composes `macro.insert` (`Session::insert_macro`, with the instance ID the session mints from the project,
//! `Session::next_macro_instance`), lists its conflicts (`Session::macro_conflicts`) and opens the `macro` card in
//! the Preview tab. `macros.value_set` previews the card again with a value set. Apply commits one transaction
//! ([`ShellModel::commit_kept`]; headless one command-log line). A card whose preview a prevent rule refused still
//! lists its conflicts, and Apply fails with the rule's text.
//!
//! **Update.** `libraries.update` previews `library.update` of the marked copies of the Libraries navigator (else the
//! copy under the cursor, else every copy with an update) and opens the bulk card (`Session::update_from_library`).
//!
//! **Save as macro… and Make variable.** A library write is a front-end operation (§13.2 item 7): no command, no
//! command log, and headless (a script, `--record`, MCP) `macros.save` refuses (`reason-library-write-window`, as the
//! purge refuses). In the window the dialog is the confirmation (D12: not undoable); a read-only library refuses before
//! anything is written. `props.make_variable` remembers the focused property row as a variable of the next save.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_api::output::ew_dl::ew_units::{Nm, Orient2, Pt2};
use ew_api::output::{MacroDetail, NavRole, NavTable, Uuid};
use ew_api::{AnyCommand, InsertAt, MacroConflict, MacroScope, MacroSpec, MacroValues, Submit};
use serde_json::{Map, Value, json};

use super::creation::{FormDialog, FormField};
use super::navigation::bump;
use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::canvas::PageId;
use crate::navigators::NavKind;
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, MacroInsert, PendingEdit};
use crate::strings;
use crate::tools::{MacroTool, PreviewState};

/// The action of Save as macro… (its dialog dispatches it).
const SAVE: &str = "macros.save";

/// A variable Make variable remembered for the next Save as macro… (doc 07 §2.7 `VarDef` with one binding).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MacroVariable {
    /// The variable's name.
    pub name: String,
    /// The record the binding names (a record of the selection; the fragment keeps the source IDs).
    pub record: Uuid,
    /// The field path `<kind>.<path>`.
    pub field: String,
    /// The variable type's word (`text`, `process_tag`, …).
    pub ty: &'static str,
}

/// The shell's macro state: the detail of the Macros navigator's cursor row, the variables of the next save.
#[derive(Default, Debug)]
pub(crate) struct MacroState {
    /// The cursor row's key and its detail (`None`: no macro row).
    detail: Option<(String, Option<Arc<MacroDetail>>)>,
    /// The variables Make variable remembered.
    variables: Vec<MacroVariable>,
}

/// The word of a variable type a property row's field takes (`Session::save_macro` checks the binding).
fn var_type(path: &str, ty: &ew_api::output::FieldType) -> &'static str {
    use ew_api::output::FieldType;
    match path {
        "process_tag" => "process_tag",
        "tag.counter" => "tag_counter",
        "state" => "state",
        _ => match ty.inner() {
            FieldType::Bool => "bool",
            FieldType::Ident => "ident",
            FieldType::Int { .. } | FieldType::Dec3 | FieldType::Quantity | FieldType::Nm => "number",
            _ => "text",
        },
    }
}

/// The conflicts of a macro card in words, the refusals first (the rule's own text).
fn conflict_words(conflicts: &[MacroConflict]) -> Vec<String> {
    let mut refused = Vec::new();
    let mut other = Vec::new();
    for c in conflicts {
        match c {
            MacroConflict::Refused { message, .. } => refused.push(message.clone()),
            MacroConflict::Tag(t) => other.push(strings::format("macro-conflict-tag", &[t])),
            MacroConflict::Pairs { name, with } => other.push(if *with == 1 {
                strings::format("macro-conflict-pairs-one", &[name])
            } else {
                strings::format("macro-conflict-pairs", &[name, &with.to_string()])
            }),
            MacroConflict::Address(a) => other.push(strings::format("macro-conflict-address", &[a])),
            MacroConflict::ProcessTag(t) => other.push(strings::format("macro-conflict-process-tag", &[t])),
        }
    }
    refused.extend(other);
    refused
}

/// The words of a row badge of the Libraries and Macros navigators (the API's codes, §11); any other badge as given.
pub(super) fn badge_word(code: &str) -> &str {
    match code {
        "update_available" => strings::text("badge-update-available"),
        "source_missing" => strings::text("badge-source-missing"),
        "deprecated" => strings::text("badge-deprecated"),
        "local_fork" => strings::text("badge-local-fork"),
        "index_failed" => strings::text("badge-index-failed"),
        other => other,
    }
}

/// A macro key from a name: lower case, every run of other characters one `-` (`PSU 24 V` → `psu-24-v`).
fn key_of(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // The Macros navigator's detail

    /// The detail of the macro under the Macros navigator's cursor (§11: name, description, "No preview", devices,
    /// variables); `None` when the cursor is on no macro row or the navigator is closed.
    pub fn macro_detail(&self) -> Option<&MacroDetail> {
        self.macros.detail.as_ref().and_then(|(_, d)| d.as_deref())
    }

    /// The variables Make variable remembered for the next Save as macro….
    pub fn macro_variables(&self) -> &[MacroVariable] {
        &self.macros.variables
    }

    /// The detail follows the Macros navigator's cursor (end of every dispatch): one `Session::macro_detail` per new
    /// cursor row.
    pub(super) fn sync_macros(&mut self) {
        let key = self
            .navs
            .contains_key(&NavKind::Macros)
            .then(|| self.nav_cursor(NavKind::Macros))
            .flatten();
        let Some(key) = key else {
            self.macros.detail = None;
            return;
        };
        if self.macros.detail.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        let detail = self
            .project
            .as_ref()
            .and_then(|p| p.session.macro_detail(&key))
            .map(Arc::new);
        self.macros.detail = Some((key, detail));
    }

    /// Drops what reads the libraries (after a library write): the tables of the Macros and Libraries navigators and
    /// the detail; the next sync fetches them again.
    fn libraries_changed(&mut self) {
        for k in [NavKind::Macros, NavKind::Libraries] {
            if let Some(n) = self.navs.get_mut(&k) {
                n.table = None;
            }
        }
        self.macros.detail = None;
    }

    /// The page UUID of the focused tab, `None` without a drawn page.
    fn macro_page(&self) -> Option<Uuid> {
        match self.current_page()? {
            PageId(ew_api::output::ew_dl::Destination::Page(p)) => Some(p),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Insert

    /// `macros.insert {key?}` (Draw ▸ Insert macro…): arms the macro tool with the macro of row `key`, else of the
    /// Macros navigator's cursor row; without either shows the Macros navigator.
    pub(super) fn act_macros_insert(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) if !k.trim().is_empty() => Some(k.trim().to_owned()),
            Ok(_) => self
                .nav_cursor(NavKind::Macros)
                .filter(|k| self.macro_row(k).is_some()),
        };
        match key {
            Some(k) => self.arm_macro(&k),
            None => {
                let mut q = Map::new();
                q.insert("kind".into(), Value::from(NavKind::Macros.word()));
                self.act_navigator_show(&q)
            }
        }
    }

    /// The node of a macro row of the Macros navigator.
    fn macro_row(&self, key: &str) -> Option<&ew_api::output::NavNode> {
        self.generic(NavKind::Macros)
            .and_then(|n| n.node_of(key))
            .map(|(_, n)| n)
            .filter(|n| n.role == NavRole::Macro)
    }

    /// Arms the macro tool with the macro of row `key` (Enter on the row, Insert macro…).
    pub(super) fn arm_macro(&mut self, key: &str) -> Outcome {
        if self.macro_page().is_none() {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        }
        let Some(detail) = self.project.as_ref().and_then(|p| p.session.macro_detail(key)) else {
            return failed("error-nav-key", &[key, NavKind::Macros.word()]);
        };
        if detail.reference.is_none() {
            return Outcome::Refused {
                reason_key: "reason-macro-not-installed",
            };
        }
        if self.navs.contains_key(&NavKind::Macros) {
            self.set_cursor(NavKind::Macros, key);
        }
        self.activate_tool(Box::new(MacroTool::new(
            key.to_owned(),
            detail.name.clone(),
            detail.extent,
        )));
        Outcome::Done
    }

    /// `macros.place {key, x_mm, y_mm, value_set?}` (a macro row dropped on the canvas; the macro tool's click): the
    /// macro's card at the grid point nearest the sheet point.
    pub(super) fn act_macros_place(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Ok(Some(k)) if !k.trim().is_empty() => k.trim().to_owned(),
            Ok(_) => return failed("error-missing-param", &["key"]),
            Err(e) => return e,
        };
        let at = match super::canvas_actions::point_mm(p) {
            Ok(Some(at)) => at,
            Ok(None) => return failed("error-missing-param", &["x_mm"]),
            Err(e) => return e,
        };
        let value_set = match string_param(p, "value_set") {
            Ok(v) => v.filter(|v| !v.is_empty()).map(str::to_owned),
            Err(e) => return e,
        };
        let g = match self.group_param(p) {
            Ok(g) => g,
            Err(e) => return e,
        };
        if let Some(a) = &mut self.area {
            a.focused = g;
        }
        let Some(c) = self.canvas() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let page_nm = c.page().snapper.snap_grid(at).page_nm;
        let Some(page) = self.macro_page() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        self.open_macro_card(&key, page, page_nm, value_set)
    }

    /// The macro tool's click (`ToolOutput::Commit` of the host step `macro.place {key, at}`, page nm): the card, and
    /// the tool returns to Select.
    pub(super) fn macro_place_from_tool(&mut self, params: &Value) -> Outcome {
        let key = params["key"].as_str().unwrap_or_default().to_owned();
        let (Some(x), Some(y)) = (params["at"][0].as_i64(), params["at"][1].as_i64()) else {
            return Outcome::Done;
        };
        let Some(page) = self.macro_page() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let out = self.open_macro_card(&key, page, (x, y), None);
        let _ = self.act_tool("select");
        out
    }

    /// Composes `macro.insert` of the macro of row `key` at `at` (page nm) on `page` with `value_set`, lists its
    /// conflicts and shows the `macro` card. A refused composition opens no card; a refused preview opens it with its
    /// conflicts.
    fn open_macro_card(
        &mut self,
        key: &str,
        page: Uuid,
        at: (i64, i64),
        value_set: Option<String>,
    ) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let submit = Submit::from(super::mode_origin(self.mode, &self.origin_file));
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let Some(detail) = project.session.macro_detail(key) else {
            return failed("error-nav-key", &[key, NavKind::Macros.word()]);
        };
        let Some(reference) = detail.reference.clone() else {
            return Outcome::Refused {
                reason_key: "reason-macro-not-installed",
            };
        };
        if let Some(v) = &value_set
            && !detail.value_sets.contains(v)
        {
            return failed("error-bad-param", &["value_set"]);
        }
        let instance = project.session.next_macro_instance(detail.copy);
        let place = InsertAt {
            page,
            at: Some(Pt2::new(Nm(at.0), Nm(at.1))),
            orient: Orient2::default(),
        };
        let values = MacroValues {
            value_set: value_set.clone(),
            values: BTreeMap::new(),
        };
        let cmd: AnyCommand = match project
            .session
            .insert_macro(&reference, &place, &values, instance)
        {
            Ok(c) => c,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let conflicts = project
            .session
            .macro_conflicts(&cmd)
            .map(|c| conflict_words(&c))
            .unwrap_or_default();
        bump(calls, |c| c.preview_with += 1);
        let result = project
            .session
            .preview_with(&cmd, submit)
            .map_err(|e| e.to_string());
        let derived = result
            .as_ref()
            .map(|p| project.session.derived_diff(p))
            .unwrap_or_default();
        let changes = result.as_ref().map_or(0, |p| p.diff().len());
        let edit = PendingEdit {
            row: String::new(),
            kind: String::new(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: cmd.name.clone(),
            params: cmd.params.clone(),
            typed: String::new(),
            label: String::new(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: cmd.params.clone(),
                name: cmd.name.clone(),
                result,
                derived,
            },
            edit,
            targets: changes,
            changes,
            reason: "reason-macro-insert",
            origin: CardOrigin::Macro(Box::new(MacroInsert {
                key: key.to_owned(),
                name: detail.name,
                page,
                at,
                instance,
                value_set,
                value_sets: detail.value_sets,
                conflicts,
            })),
        });
        Outcome::Done
    }

    /// `macros.value_set {name?}`: previews the open macro card again with the value set `name` (absent or empty: the
    /// defaults).
    pub(super) fn act_macros_value_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Ok(v) => v.filter(|v| !v.is_empty()).map(str::to_owned),
            Err(e) => return e,
        };
        let Some(CardOrigin::Macro(m)) = self.props.card().map(|c| c.origin.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        self.open_macro_card(&m.key, m.page, m.at, name)
    }

    /// Apply on a macro card: a fresh card commits one transaction; a refused one fails with the rule's text; a stale
    /// one is composed again (a second Apply).
    pub(super) fn apply_macro_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            let message = message.clone();
            self.props.card = Some(card);
            return Outcome::Failed { message };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            return self.commit_kept(preview, &edit.name, &edit.params, None);
        }
        let CardOrigin::Macro(m) = card.origin else {
            return Outcome::Done;
        };
        self.open_macro_card(&m.key, m.page, m.at, m.value_set)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Update

    /// The Libraries navigator's table: the open navigator's, else one read.
    fn libraries_table(&self) -> Option<Arc<NavTable>> {
        if let Some(t) = self.generic(NavKind::Libraries).and_then(|n| n.table()) {
            return Some(Arc::clone(t));
        }
        let project = self.project.as_ref()?;
        bump(&self.calls, |c| c.navigator_table += 1);
        Some(Arc::new(project.session.navigator_table(NavKind::Libraries)))
    }

    /// `libraries.update {keys?}` (the Libraries navigator's Update…): previews `library.update` of the copies of rows
    /// `keys`, else of the marked copy rows, else of the copy row under the cursor, else of every copy with an update,
    /// and opens the bulk card.
    pub(super) fn act_libraries_update(&mut self, p: &Map<String, Value>) -> Outcome {
        let keys = match super::table::list_param(p, "keys") {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(table) = self.libraries_table() else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let copy_rows: Vec<&ew_api::output::NavNode> = table
            .nodes
            .iter()
            .filter(|n| n.role == NavRole::LibCopy)
            .collect();
        let by_key = |k: &str| copy_rows.iter().find(|n| n.key == k).copied();
        let chosen: Vec<&ew_api::output::NavNode> = if let Some(keys) = keys {
            let mut out = Vec::new();
            for k in &keys {
                match by_key(k) {
                    Some(n) => out.push(n),
                    None => return failed("error-nav-key", &[k, NavKind::Libraries.word()]),
                }
            }
            out
        } else {
            let nav = self.generic(NavKind::Libraries);
            let marked: Vec<_> = nav
                .map(|n| n.marked().iter().filter_map(|k| by_key(k)).collect())
                .unwrap_or_default();
            if !marked.is_empty() {
                marked
            } else if let Some(n) = self.nav_cursor(NavKind::Libraries).and_then(|k| by_key(&k)) {
                vec![n]
            } else {
                copy_rows
                    .iter()
                    .copied()
                    .filter(|n| n.badges.iter().any(|b| b == "update_available"))
                    .collect()
            }
        };
        let copies: Vec<Uuid> = chosen
            .iter()
            .filter(|n| n.badges.iter().any(|b| b == "update_available"))
            .filter_map(|n| n.records.first().copied())
            .collect();
        if copies.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-updates",
            };
        }
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let plan = match project.session.update_from_library(&copies, &[]) {
            Ok(plan) => plan,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        if plan.copies.is_empty() {
            return Outcome::Refused {
                reason_key: "reason-no-updates",
            };
        }
        self.preview_host_card(&plan.command.name, plan.command.params, "reason-library-update")
    }

    // -----------------------------------------------------------------------------------------------------------
    // Make variable and Save as macro…

    /// `props.make_variable {row?, name?}` (the property row's Make variable): remembers the row's field of the one
    /// record it shows as a variable of the next Save as macro… (`name` default: the field's last member).
    pub(super) fn act_props_make_variable(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match self.props_row_param(p) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some((section, row)) = self.props_find(&key) else {
            return failed("error-no-row", &[&key]);
        };
        let [record] = section.targets.as_slice() else {
            return Outcome::Refused {
                reason_key: "reason-variable-one-record",
            };
        };
        if row.path.starts_with("derived.") {
            return Outcome::Refused {
                reason_key: "reason-variable-derived",
            };
        }
        let field = format!("{}.{}", section.kind, row.path);
        let default_name = key_of(row.path.rsplit('.').next().unwrap_or(&row.path)).replace('-', "_");
        let name = match string_param(p, "name") {
            Ok(Some(n)) if !n.trim().is_empty() => n.trim().to_owned(),
            Ok(_) => default_name,
            Err(e) => return e,
        };
        let var = MacroVariable {
            name: name.clone(),
            record: *record,
            field,
            ty: var_type(&row.path, &row.ty),
        };
        self.macros
            .variables
            .retain(|v| v.name != var.name && (v.record, &v.field) != (var.record, &var.field));
        self.macros.variables.push(var);
        self.tools.announcement = Some(strings::format("announce-variable", &[&name]));
        Outcome::Done
    }

    /// Whether the open dialog is Save as macro….
    fn in_save_form(&self) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == SAVE)
    }

    /// The end of Save as macro…: a failure or refusal stays in its dialog with its text, a success closes it.
    fn save_done(&mut self, out: Outcome) -> Outcome {
        if self.in_save_form() {
            let text = match &out {
                Outcome::Done => {
                    self.dialog = None;
                    None
                }
                Outcome::Failed { message } => Some(message.clone()),
                Outcome::Refused { reason_key } => Some(strings::text(reason_key).to_owned()),
            };
            if let (Some(text), Some(Dialog::Form(f))) = (text, &mut self.dialog) {
                f.error = Some(text);
            }
        }
        out
    }

    /// `macros.save {name?, key?, description?, library?, confirm?}` (Edit ▸ Save as macro…): writes the selection as
    /// a window macro into a writable library (by name, key or ID; default the first writable one). Without `name`
    /// or `confirm` the window opens the dialog, which is the confirmation (the write is not undoable, D12); headless
    /// the action refuses (a library write is a front-end operation, §13.2 item 7).
    pub(super) fn act_macros_save(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: "reason-library-write-window",
            };
        }
        let text = |name: &str| -> Result<String, Outcome> {
            Ok(string_param(p, name)?
                .map(|s| s.trim().to_owned())
                .unwrap_or_default())
        };
        let (name, key, description, library) =
            match (text("name"), text("key"), text("description"), text("library")) {
                (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
                (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => {
                    return self.save_done(e);
                }
            };
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c || self.in_save_form(),
            Err(e) => return self.save_done(e),
        };
        let scope: Vec<Uuid> = self.copy_targets().iter().map(|e| e.0).collect();
        if scope.is_empty() {
            return self.save_done(Outcome::Refused {
                reason_key: "reason-nothing-selected",
            });
        }
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let libraries = project.session.libraries().libraries();
        if !confirm || name.is_empty() {
            if self.in_save_form() {
                return self.save_done(failed("error-missing-param", &["name"]));
            }
            let library = if library.is_empty() {
                libraries
                    .iter()
                    .find(|l| l.writable)
                    .map(|l| l.name.clone())
                    .unwrap_or_default()
            } else {
                library
            };
            let field = |name: &'static str, label_key: &'static str, value: String| FormField {
                name,
                label_key,
                choices: &[],
                value,
            };
            let form = FormDialog {
                action: SAVE,
                title_key: "save-macro-title",
                ok_key: "dialog-save-macro",
                fields: vec![
                    field("name", "form-macro-name", name),
                    field("key", "form-macro-key", key),
                    field("description", "form-macro-description", description),
                    field("library", "form-macro-library", library),
                ],
                error: None,
            };
            return self.open_dialog(Dialog::Form(form));
        }
        let wanted = library.to_lowercase();
        let Some(target) = libraries.iter().find(|l| {
            wanted.is_empty() && l.writable
                || l.name.to_lowercase() == wanted
                || l.key == library
                || l.id.to_string() == library
        }) else {
            // FX38b: an empty library field means "the first writable one"; with none installed that is the reason,
            // not a library called "".
            if library.is_empty() {
                return self.save_done(Outcome::Refused {
                    reason_key: "reason-no-writable-library",
                });
            }
            return self.save_done(failed("error-no-library", &[&library]));
        };
        if !target.writable {
            return self.save_done(Outcome::Refused {
                reason_key: "reason-library-read-only",
            });
        }
        let (target, target_name) = (target.id, target.name.clone());
        let key = if key.is_empty() { key_of(&name) } else { key };
        // FX38b: a name without an ASCII letter or digit gives no key; refused in words before any write, not left
        // to the library's identifier error.
        if key.is_empty() {
            return self.save_done(Outcome::Refused {
                reason_key: "reason-macro-no-key",
            });
        }
        let variables = match serde_json::from_value(Value::Array(
            self.macros
                .variables
                .iter()
                .map(|v| {
                    json!({"name": v.name, "ty": v.ty,
                        "bindings": [{"record": v.record.to_string(), "field": v.field}]})
                })
                .collect(),
        )) {
            Ok(v) => v,
            Err(e) => return self.save_done(failed("error-macro-save", &[&e.to_string()])),
        };
        let (Ok(name_text), Ok(description_text)) = (
            serde_json::from_value(Value::String(name.clone())),
            serde_json::from_value(Value::String(description)),
        ) else {
            return self.save_done(failed("error-bad-param", &["name"]));
        };
        let spec = MacroSpec {
            key,
            name: name_text,
            description: description_text,
            scope: MacroScope::Window(scope),
            origin: None,
            keep_tags: Vec::new(),
            variables,
            groups: Vec::new(),
            value_sets: Vec::new(),
        };
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let saved = match project.session.save_macro(target, &spec) {
            Ok(s) => s,
            Err(e) => return self.save_done(failed("error-macro-save", &[&e.to_string()])),
        };
        self.macros.variables.clear();
        self.libraries_changed();
        self.notes.push(
            strings::format("note-macro-saved", &[&name, &saved.version, &target_name]),
            "library",
            None,
        );
        self.save_done(Outcome::Done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_lower_case_words_joined_by_dashes() {
        assert_eq!(key_of("PSU 24 V DC"), "psu-24-v-dc");
        assert_eq!(key_of("  Motor (DOL) "), "motor-dol");
        // ASCII only (FX38b): a non-ASCII letter is dropped, and a name of none gives the empty key `macros.save`
        // refuses.
        assert_eq!(key_of("Übersicht"), "bersicht");
        assert_eq!(key_of("Ωμέγα"), "");
    }

    #[test]
    fn refusals_come_first_in_words() {
        let words = conflict_words(&[
            MacroConflict::Pairs {
                name: "L-1".into(),
                with: 2,
            },
            MacroConflict::Refused {
                rule: "EW-CORE-052".into(),
                message: "a second source".into(),
            },
            MacroConflict::Pairs {
                name: "N-1".into(),
                with: 1,
            },
        ]);
        assert_eq!(words[0], "a second source");
        assert!(words[1].contains("L-1") && words[1].contains('2'), "{words:?}");
        assert!(words[2].contains("N-1"), "{words:?}");
    }
}
