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
use ew_api::output::{LibraryCopyDetail, MacroDetail, MacroThumbnail, NavRole, NavTable, Uuid};
use ew_api::{AnyCommand, InsertAt, MacroConflict, MacroScope, MacroSpec, MacroValues, Submit};
use serde_json::{Map, Value, json};

use super::creation::{FormDialog, FormField, FormRow, FormTable};
use super::navigation::bump;
use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::canvas::PageId;
use crate::navigators::NavKind;
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, MacroField, MacroInsert, PendingEdit};
use crate::strings;
use crate::tools::{MacroTool, PreviewState};

/// The action of Save as macro… (its dialog dispatches it).
const SAVE: &str = "macros.save";

/// The kinds Save as macro… offers (FT-754 (b)).
const MACRO_KINDS: [&str; 2] = ["window", "page"];

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

/// A macro's thumbnail, shared.
type Thumbnail = Arc<MacroThumbnail>;

/// The shell's macro state: the detail of the Macros navigator's cursor row, the variables of the next save.
#[derive(Default, Debug)]
pub(crate) struct MacroState {
    /// The cursor row's key, its detail (`None`: no macro row) and its thumbnail (`None`: none drawn, FT-754 (a)).
    detail: Option<(String, Option<Arc<MacroDetail>>, Option<Thumbnail>)>,
    /// The variables Make variable remembered.
    variables: Vec<MacroVariable>,
    /// The Libraries navigator's cursor row key, the table it was read from (its address: a new table after a change
    /// reads it again) and the copy's detail (`None`: no copy row) (FT-755).
    copy_detail: Option<(String, usize, Option<Arc<LibraryCopyDetail>>)>,
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

/// A typed text as the JSON value `macro.insert` coerces for a variable of type `ty` (FT-785, FT-786): `bool`
/// `true`/`yes` and `false`/`no` (any case) a boolean; `tag_counter` and `channel_binding` digits an integer; `number`
/// a decimal text with a comma taken as the point; `part` a copy ID `{"copy": <ID>}`; anything else the text. A text
/// that does not read as its type stays a text, which the command then refuses in words.
fn literal_value(ty: &str, text: &str) -> Value {
    let text = text.trim();
    match ty {
        "bool" => match text.to_lowercase().as_str() {
            "true" | "yes" => Value::Bool(true),
            "false" | "no" => Value::Bool(false),
            _ => Value::from(text),
        },
        "tag_counter" | "channel_binding" => text
            .parse::<u64>()
            .map_or_else(|_| Value::from(text), Value::from),
        "number" => Value::from(text.replace(',', ".")),
        "part" if text.parse::<Uuid>().is_ok() => json!({ "copy": text }),
        _ => Value::from(text),
    }
}

/// A variable's value as the card and the value-set table show it: a text as it is, a part by its copy ID, any other
/// value as its JSON text.
fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(o) => match o.get("copy") {
            Some(Value::String(c)) => c.clone(),
            _ => v.to_string(),
        },
        other => other.to_string(),
    }
}

/// The typed values of a macro card's fields by variable name (blank fields left out).
fn literals(m: &MacroInsert) -> BTreeMap<String, String> {
    m.fields
        .iter()
        .filter(|f| !f.value.trim().is_empty())
        .map(|f| (f.name.clone(), f.value.clone()))
        .collect()
}

/// One value set of the `value_sets` parameter: its name and its values by variable name.
type ValueSetParam = (String, Map<String, Value>);

/// The `value_sets` parameter of `macros.save` (FT-786): an array of `{"name": <text>, "values": {<variable>:
/// <value>}}`, `None` when absent.
fn value_sets_param(p: &Map<String, Value>) -> Result<Option<Vec<ValueSetParam>>, Outcome> {
    let bad = || failed("error-bad-param", &["value_sets"]);
    match p.get("value_sets") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(rows)) => rows
            .iter()
            .map(|r| {
                let name = r.get("name").and_then(Value::as_str).ok_or_else(bad)?;
                let values = match r.get("values") {
                    None | Some(Value::Null) => Map::new(),
                    Some(Value::Object(o)) => o.clone(),
                    Some(_) => return Err(bad()),
                };
                Ok((name.to_owned(), values))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(bad()),
    }
}

/// A refused save in words (FT-786): a value set's refusal names the set and, for a cell, the variable; any other
/// refusal as the library gave it. `sets` are the value sets' names in the order saved.
fn save_error(e: &ew_api::MacroError, sets: &[String]) -> Outcome {
    if let ew_api::MacroError::Spec { at, message } = e
        && let Some(rest) = at.strip_prefix("value_sets/")
    {
        let (index, variable) = match rest.split_once('/') {
            Some((i, v)) => (i, Some(v)),
            None => (rest, None),
        };
        if let Some(set) = index.parse::<usize>().ok().and_then(|i| sets.get(i)) {
            let message = match variable {
                Some(v) => strings::format("error-value-set-cell", &[set, v, message]),
                None => strings::format("error-value-set", &[set, message]),
            };
            return Outcome::Failed { message };
        }
    }
    failed("error-macro-save", &[&e.to_string()])
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

/// A macro key from a name: lower case, letters and digits of any script kept, every run of other characters one `-`
/// (`PSU 24 V` → `psu-24-v`, `Übersicht` → `übersicht`). The name is NFC-normalised first, so a name typed with
/// combining marks (NFD) gives the same key as its composed form (an `Ident` is NFC).
fn key_of(name: &str) -> String {
    let name = ew_api::to_nfc(name);
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
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
        self.macros.detail.as_ref().and_then(|(_, d, _)| d.as_deref())
    }

    /// The thumbnail of the macro under the Macros navigator's cursor (FT-754 (a): the macro drawn on a blank page of
    /// a throwaway copy, `Session::macro_thumbnail`); `None` when there is none, and the detail then says "No
    /// preview".
    pub fn macro_thumbnail(&self) -> Option<&MacroThumbnail> {
        self.macros.detail.as_ref().and_then(|(_, _, t)| t.as_deref())
    }

    /// The detail of the library copy under the Libraries navigator's cursor (FT-755: name, kind, key, library,
    /// version, state, as the Macros navigator's detail shows a macro); `None` when the cursor is on no copy row or
    /// the navigator is closed.
    pub fn library_copy_detail(&self) -> Option<&LibraryCopyDetail> {
        self.macros
            .copy_detail
            .as_ref()
            .and_then(|(_, _, d)| d.as_deref())
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
        if self.macros.detail.as_ref().is_some_and(|(k, ..)| *k == key) {
            return;
        }
        let session = self.project.as_ref().map(|p| &p.session);
        let detail = session.and_then(|s| s.macro_detail(&key)).map(Arc::new);
        let thumbnail = session
            .filter(|_| detail.is_some())
            .and_then(|s| s.macro_thumbnail(&key))
            .map(Arc::new);
        self.macros.detail = Some((key, detail, thumbnail));
    }

    /// The copy detail follows the Libraries navigator's cursor and table (end of every dispatch, FT-755): one
    /// `Session::library_copy_detail` per new cursor row or new table.
    pub(super) fn sync_library_copy(&mut self) {
        let key = self
            .navs
            .contains_key(&NavKind::Libraries)
            .then(|| self.nav_cursor(NavKind::Libraries))
            .flatten();
        let Some(key) = key else {
            self.macros.copy_detail = None;
            return;
        };
        let table = self
            .generic(NavKind::Libraries)
            .and_then(|n| n.table())
            .map_or(0, |t| Arc::as_ptr(t) as usize);
        if self
            .macros
            .copy_detail
            .as_ref()
            .is_some_and(|(k, t, _)| *k == key && *t == table)
        {
            return;
        }
        let detail = self
            .project
            .as_ref()
            .and_then(|p| p.session.library_copy_detail(&key))
            .map(Arc::new);
        self.macros.copy_detail = Some((key, table, detail));
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
        self.macros.copy_detail = None;
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
        self.open_macro_card(&key, page, page_nm, value_set, BTreeMap::new())
    }

    /// The macro tool's click ([`crate::tools::HostStep::MacroPlace`] of row `key` at `at`, page nm): the card, and
    /// the tool returns to Select.
    pub(super) fn macro_place_from_tool(&mut self, key: &str, at: (i64, i64)) -> Outcome {
        let Some(page) = self.macro_page() else {
            return Outcome::Refused {
                reason_key: "reason-no-page",
            };
        };
        let out = self.open_macro_card(key, page, at, None, BTreeMap::new());
        let _ = self.act_tool("select");
        out
    }

    /// Composes `macro.insert` of the macro of row `key` at `at` (page nm) on `page` with `value_set` and the typed
    /// values `typed` (by variable name; FT-785: literals win over the value set and the defaults), lists its
    /// conflicts and shows the `macro` card. A refused composition opens no card; a refused preview opens it with its
    /// conflicts, and a typed value the command does not take shows the command's reason under its field.
    fn open_macro_card(
        &mut self,
        key: &str,
        page: Uuid,
        at: (i64, i64),
        value_set: Option<String>,
        typed: BTreeMap<String, String>,
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
        // FT-785: one field per variable; its placeholder is the chosen set's value, else the default.
        let set_values = value_set
            .as_ref()
            .and_then(|v| detail.value_sets.iter().position(|n| n == v))
            .and_then(|i| detail.value_set_values.get(i));
        let mut fields: Vec<MacroField> = detail
            .variable_fields
            .iter()
            .map(|(name, ty, default)| MacroField {
                name: name.clone(),
                ty: ty.clone(),
                placeholder: set_values
                    .and_then(|s| s.get(name))
                    .or(default.as_ref())
                    .map(value_text)
                    .unwrap_or_default(),
                value: typed.get(name).cloned().unwrap_or_default(),
                error: None,
            })
            .collect();
        let values = MacroValues {
            value_set: value_set.clone(),
            values: fields
                .iter()
                .filter(|f| !f.value.trim().is_empty())
                .map(|f| (f.name.clone(), literal_value(&f.ty, &f.value)))
                .collect(),
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
        let raw = project.session.preview_with(&cmd, submit);
        // A typed value the command does not take: its reason under the field, without the variable's name the field
        // already shows.
        if let Err(ew_api::CmdError::InvalidParams { path, message, .. }) = &raw
            && let Some(name) = path.strip_prefix("/values/")
            && let Some(f) = fields.iter_mut().find(|f| f.name == name)
        {
            let lead = format!("variable `{name}`: ");
            f.error = Some(message.strip_prefix(&lead).unwrap_or(message).to_owned());
        }
        let result = raw.map_err(|e| e.to_string());
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
                fields,
                conflicts,
            })),
        });
        Outcome::Done
    }

    /// `macros.value_set {name?}`: previews the open macro card again with the value set `name` (absent or empty: the
    /// defaults); the typed values stay.
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
        let typed = literals(&m);
        self.open_macro_card(&m.key, m.page, m.at, name, typed)
    }

    /// `macros.value {name, value?}` (FT-785): previews the open macro card again with `value` typed for its variable
    /// `name` (absent or blank: none, the value set's or the default's value applies); the value set and the other
    /// typed values stay.
    pub(super) fn act_macros_value(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Ok(Some(n)) if !n.trim().is_empty() => n.trim().to_owned(),
            Ok(_) => return failed("error-missing-param", &["name"]),
            Err(e) => return e,
        };
        let value = match string_param(p, "value") {
            Ok(v) => v.map(|v| v.trim().to_owned()).unwrap_or_default(),
            Err(e) => return e,
        };
        let Some(CardOrigin::Macro(m)) = self.props.card().map(|c| c.origin.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-card",
            };
        };
        if !m.fields.iter().any(|f| f.name == name) {
            return failed("error-bad-param", &["name"]);
        }
        let mut typed = literals(&m);
        if value.is_empty() {
            typed.remove(&name);
        } else {
            typed.insert(name, value);
        }
        self.open_macro_card(&m.key, m.page, m.at, m.value_set.clone(), typed)
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
        let typed = literals(&m);
        self.open_macro_card(&m.key, m.page, m.at, m.value_set, typed)
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

    /// The pages of a page macro (FT-754 (b)): the folios of `pages`, else the marked pages of the Pages navigator
    /// (else its cursor's page), as page IDs in page order; generated pages refuse.
    fn macro_pages(&self, p: &Map<String, Value>) -> Result<Vec<Uuid>, Outcome> {
        let folios = super::table::list_param(p, "pages")?;
        match folios.filter(|f| !f.is_empty()) {
            Some(folios) => {
                let rows = self.page_rows();
                folios
                    .iter()
                    .map(|f| match rows.iter().find(|r| r.folio == *f) {
                        Some(r) if !r.generated && r.kind != "section_divider" => Ok(r.id),
                        Some(_) => Err(failed("error-generated-page-order", &[])),
                        None => Err(failed("error-no-folio", &[f])),
                    })
                    .collect()
            }
            None => Ok(self.marked_pages()?.into_iter().map(|r| r.id).collect()),
        }
    }

    /// Whether the open dialog is Save as macro….
    fn in_save_form(&self) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == SAVE)
    }

    /// The end of Save as macro…: a failure or refusal stays in its dialog with its text, a success closes it. A
    /// refusal before the dialog opens (nothing selected, no project) shows its reason in the banner (FT-755: a refused
    /// save leaves a trace in the window; a failure becomes the banner through the window's report).
    fn save_done(&mut self, out: Outcome) -> Outcome {
        if !self.in_save_form()
            && let Outcome::Refused { reason_key } = &out
        {
            self.show_banner(strings::text(reason_key));
        }
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

    /// `macros.save {name?, key?, description?, library?, kind?, pages?, x_mm?, y_mm?, confirm?}` (Edit ▸ Save as
    /// macro…): writes the selection as a window macro, or pages as a page macro, into a writable library (by name,
    /// key or ID; default the first writable one). `kind` is `window` or `page` (FT-754 (b)); without it a selection
    /// of drawn records makes a window macro, and no selection or one of pages only (marking pages selects them) a
    /// page macro. A page macro's pages are
    /// `pages` (folios), else the marked pages (else the Pages cursor's page). A window macro's insertion point is
    /// `x_mm`/`y_mm` (a sheet point of the page on show, snapped to its grid), else the selection's anchor. Without
    /// `name` or `confirm` the window opens the dialog (name, key, description, library, kind, insertion point), which
    /// is the confirmation (the write is not undoable, D12); headless the action refuses (a library write is a
    /// front-end operation, §13.2 item 7).
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
        let kind = match text("kind") {
            Ok(k) => k.to_lowercase(),
            Err(e) => return self.save_done(e),
        };
        // The insertion point's coordinates: a number (a script) or a text (the dialog's field), mm.
        let coordinate = |name: &str| -> Result<Option<f64>, Outcome> {
            match p.get(name) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::Number(n)) => Ok(n.as_f64().filter(|v| v.is_finite())),
                Some(Value::String(t)) if t.trim().is_empty() => Ok(None),
                Some(Value::String(t)) => t
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(Some)
                    .ok_or_else(|| failed("error-bad-param", &[name])),
                Some(_) => Err(failed("error-bad-param", &[name])),
            }
        };
        let (x_mm, y_mm) = match (coordinate("x_mm"), coordinate("y_mm")) {
            (Ok(x), Ok(y)) => (x, y),
            (Err(e), _) | (_, Err(e)) => return self.save_done(e),
        };
        let mm_text = |v: Option<f64>| v.map(|v| format!("{v}")).unwrap_or_default();
        let (x_text, y_text) = (mm_text(x_mm), mm_text(y_mm));
        let value_sets = match value_sets_param(p) {
            Ok(v) => v.unwrap_or_default(),
            Err(e) => return self.save_done(e),
        };
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c || self.in_save_form(),
            Err(e) => return self.save_done(e),
        };
        let selection: Vec<Uuid> = self.copy_targets().iter().map(|e| e.0).collect();
        let page_macro = match kind.as_str() {
            "window" => false,
            "page" => true,
            // No kind: pages (nothing selected, or only pages, as marking pages selects them) make a page macro.
            "" => {
                let rows = self.page_rows();
                selection.iter().all(|id| rows.iter().any(|r| r.id == *id))
            }
            _ => return self.save_done(failed("error-bad-param", &["kind"])),
        };
        let scope = if page_macro {
            match self.macro_pages(p) {
                Ok(pages) => MacroScope::Pages(pages),
                Err(e) => return self.save_done(e),
            }
        } else if selection.is_empty() {
            return self.save_done(Outcome::Refused {
                reason_key: "reason-nothing-selected",
            });
        } else {
            MacroScope::Window(selection)
        };
        // The insertion point: the form's texts or the parameters' numbers (a sheet point in mm), snapped to the page
        // grid; a page macro has none.
        let origin = if page_macro {
            None
        } else {
            match (x_mm, y_mm) {
                (Some(x), Some(y)) => {
                    let at = ((x * 1_000_000.0).round() as i64, (y * 1_000_000.0).round() as i64);
                    self.canvas().map(|c| {
                        let (px, py) = c.page().snapper.snap_grid(at).page_nm;
                        Pt2::new(Nm(px), Nm(py))
                    })
                }
                (None, None) => None,
                (None, _) => return self.save_done(failed("error-missing-param", &["x_mm"])),
                (_, None) => return self.save_done(failed("error-missing-param", &["y_mm"])),
            }
        };
        if self.project.is_none() {
            return self.save_done(Outcome::Refused {
                reason_key: "reason-no-project",
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
            let mut fields = vec![
                field("name", "form-macro-name", name),
                field("key", "form-macro-key", key),
                field("description", "form-macro-description", description),
                field("library", "form-macro-library", library),
                FormField {
                    name: "kind",
                    label_key: "form-macro-kind",
                    choices: &MACRO_KINDS,
                    value: if page_macro { "page" } else { "window" }.to_owned(),
                },
            ];
            // A window macro's insertion point, prefilled with the point the save would take (empty: the selection's
            // anchor).
            if !page_macro {
                fields.push(field("x_mm", "form-macro-x", x_text));
                fields.push(field("y_mm", "form-macro-y", y_text));
            }
            // FT-786: the value-set table, one column per variable of the next save, prefilled from `value_sets`.
            let columns: Vec<(String, String)> = self
                .macros
                .variables
                .iter()
                .map(|v| (v.name.clone(), v.ty.to_owned()))
                .collect();
            let rows = value_sets
                .iter()
                .map(|(name, values)| FormRow {
                    key: name.clone(),
                    cells: columns
                        .iter()
                        .map(|(c, _)| values.get(c).map(value_text).unwrap_or_default())
                        .collect(),
                })
                .collect();
            let form = FormDialog {
                action: SAVE,
                title_key: "save-macro-title",
                ok_key: "dialog-save-macro",
                fields,
                error: None,
                table: Some(FormTable {
                    name: "value_sets",
                    key_label: "form-value-set-name",
                    columns,
                    rows,
                }),
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
        // FX38b: a name without a letter or digit gives no key; refused in words before any write, not left
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
        // FT-786: the value sets, each cell read as its variable's type; a set needs a name.
        let mut set_names = Vec::new();
        let mut sets = Vec::new();
        for (i, (set, values)) in value_sets.iter().enumerate() {
            let set = ew_api::to_nfc(set.trim()).into_owned();
            if set.is_empty() {
                return self.save_done(failed("error-value-set-name", &[&(i + 1).to_string()]));
            }
            let values: Map<String, Value> = values
                .iter()
                .filter(|(_, v)| v.as_str().is_none_or(|s| !s.trim().is_empty()))
                .map(|(var, v)| {
                    let ty = self
                        .macros
                        .variables
                        .iter()
                        .find(|x| x.name == *var)
                        .map_or("text", |x| x.ty);
                    let v = match v {
                        Value::String(s) => literal_value(ty, s),
                        other => other.clone(),
                    };
                    (var.clone(), v)
                })
                .collect();
            sets.push(json!({ "name": set, "values": values }));
            set_names.push(set);
        }
        let value_sets = match serde_json::from_value(Value::Array(sets)) {
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
            scope,
            origin,
            keep_tags: Vec::new(),
            variables,
            groups: Vec::new(),
            value_sets,
        };
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let saved = match project.session.save_macro(target, &spec) {
            Ok(s) => s,
            Err(e) => return self.save_done(save_error(&e, &set_names)),
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
        // FT-763: letters and digits of any script are kept; a name of none gives the empty key `macros.save` refuses.
        assert_eq!(key_of("Übersicht"), "übersicht");
        assert_eq!(key_of("Ωμέγα"), "ωμέγα");
        assert_eq!(key_of("Schütz K1"), "schütz-k1");
        assert_eq!(key_of("  "), "");
        assert_eq!(key_of("--"), "");
        // FIX39a (finding 8): a decomposed name (U + combining diaeresis) gives the composed name's key.
        assert_eq!(key_of("U\u{308}bersicht"), "übersicht");
        assert_eq!(key_of("Schu\u{308}tz K1"), key_of("Schütz K1"));
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
