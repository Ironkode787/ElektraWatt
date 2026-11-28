// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Many parts at once in Parts (doc 07 §5.9): several rows of the list selected (`parts.open {part, add}` toggles one,
//! `{part, range}` takes the rows from the open part to this one) show the fields they share on one sheet, each with
//! the value they all have or "mixed"; `parts.field` types a value into all of them; `parts.save` saves them together,
//! the card of what changes for the project naming how many parts and which fields before its Apply, one undo step;
//! `parts.discard` drops what was typed. `parts.export {path?}` writes the selected parts, or the open one, as a table
//! of their declared fields, which Import… reads back as the same.

use ew_api::output::{CsvOptions, Preset, Table, TableCell, TableColumn, TableRow};
use ew_api::parts::Lives;
use ew_api::{PartDraftError, PartSheet};
use serde_json::{Map, Value};

use super::part_dialog::{card_fields, draft_refusal};
use super::parts_place::lives_word;
use crate::key;
use crate::properties::{CardOrigin, PartCard};
use crate::shell::{Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::strings;

/// Several parts selected in the list, typed into together (doc 07 §5.9).
#[derive(Clone, PartialEq, Debug)]
pub struct ManySheet {
    /// The parts, as `parts.open` names them (`id:…`, `lib:…/…`), in the order they were selected.
    pub parts: Vec<String>,
    /// Each part's sheet and where it lives, in that order.
    pub sheets: Vec<(PartSheet, Lives)>,
    /// The fields they share, as the sheet names them, each with the text they all have or `None` ("mixed").
    pub fields: Vec<(String, Option<String>)>,
    /// Whether a value was typed since they were opened or saved.
    pub changed: bool,
    /// The refusal of the last Save or of a value, in words.
    pub error: Option<String>,
}

impl ManySheet {
    /// The parts that are not the person's to change (a starter part, a library he cannot write), by number.
    pub fn not_his(&self) -> Vec<String> {
        self.sheets
            .iter()
            .filter(|(_, l)| matches!(l, Lives::NotMine { .. }))
            .map(|(s, _)| s.text("part_number").to_owned())
            .collect()
    }

    /// A shared field in words: a cell of the Connections table by its pin and its column (`Pin 1 · Clamp`), else
    /// [`field_words`].
    pub fn words(&self, field: &str) -> String {
        let cell = field
            .split_once('[')
            .and_then(|(table, rest)| rest.split_once("].").map(|(row, column)| (table, row, column)));
        let pin = cell.and_then(|(table, row, column)| {
            let key = self
                .sheets
                .first()?
                .0
                .table(table)?
                .keys
                .get(row.parse::<usize>().ok()?)?;
            Some((key.clone(), field_words(&format!("{table}[].{column}"))))
        });
        match pin {
            Some((pin, column)) => strings::format(key!("parts-many-cell"), &[&pin, &column]),
            None => field_words(field),
        }
    }

    /// Each part's number and where it lives in words.
    pub fn numbers(&self) -> Vec<(String, String)> {
        self.sheets
            .iter()
            .map(|(s, l)| (s.text("part_number").to_owned(), lives_word(l)))
            .collect()
    }
}

/// A field of a sheet in words: a declared field's word, a named technical value's, else the name.
pub fn field_words(field: &str) -> String {
    let path = match field.strip_prefix("tech:") {
        Some(key) => format!("tech_data.{key}"),
        None => field.to_owned(),
    };
    ew_api::part_fields::field(&path).map_or_else(
        || field.to_owned(),
        |f| strings::render(&ew_api::part_fields::word(f.word)),
    )
}

impl ShellModel {
    /// The parts selected together, while there are several.
    pub fn many_sheet(&self) -> Option<&ManySheet> {
        self.parts.as_deref()?.many.as_deref()
    }

    /// The sheet of the part `param` names (`id:…`, `lib:…/…`).
    fn sheet_named(&self, param: &str) -> Option<PartSheet> {
        let session = self.parts_session()?;
        if let Some(id) = param.strip_prefix("id:") {
            return session.part_draft(id.parse().ok()?).ok();
        }
        let (library, item) = param.strip_prefix("lib:")?.split_once('/')?;
        session
            .library_part_draft(library.parse().ok()?, item.parse().ok()?)
            .ok()
    }

    /// The open part as `parts.open` names it, where the list does not show it (a search that leaves it out): the
    /// project's copy where the project holds it, else the library's item.
    fn sheet_param(&self, sheet: &PartSheet) -> Option<String> {
        let session = self.parts_session()?;
        if session.part_draft(sheet.copy).is_ok() {
            return Some(format!("id:{}", sheet.copy));
        }
        sheet
            .source
            .map(|(library, item)| format!("lib:{library}/{item}"))
    }

    /// `parts.open {part, add?, range?}` of a row of the list with Ctrl (`add`, the part in or out of the selection) or
    /// Shift (`range`, the rows from the open part to it, in the list's order): two or more parts make the sheet of
    /// many; one left opens as its own sheet.
    pub(super) fn select_many(&mut self, part: &str, range: bool) -> Outcome {
        let Some(place) = self.parts.as_deref() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        if place.sheet.as_ref().is_some_and(|o| o.changed) || place.many.as_ref().is_some_and(|m| m.changed) {
            return Outcome::refused(key!("reason-parts-unsaved"));
        }
        let mut chosen: Vec<String> = match (&place.many, &place.sheet) {
            (Some(m), _) => m.parts.clone(),
            (None, Some(open)) => place
                .found
                .iter()
                .find(|f| f.copy == open.sheet.copy)
                .map(ew_api::parts::PartFound::param)
                .or_else(|| self.sheet_param(&open.sheet))
                .into_iter()
                .collect(),
            (None, None) => Vec::new(),
        };
        // The rows as the list shows them: a range takes no row the list does not show between its ends.
        let order = place.shown_order();
        if range {
            let from = chosen.last().and_then(|c| order.iter().position(|o| o == c));
            let to = order.iter().position(|o| o == part);
            match (from, to) {
                (Some(a), Some(b)) => {
                    for p in &order[a.min(b)..=a.max(b)] {
                        if !chosen.contains(p) {
                            chosen.push(p.clone());
                        }
                    }
                }
                _ => chosen.push(part.to_owned()),
            }
        } else if let Some(i) = chosen.iter().position(|c| c == part) {
            chosen.remove(i);
        } else {
            chosen.push(part.to_owned());
        }
        match chosen.as_slice() {
            [] => {
                if let Some(place) = self.parts.as_deref_mut() {
                    place.many = None;
                    place.sheet = None;
                }
                Outcome::Done
            }
            [one] => {
                let one = one.clone();
                if let Some(place) = self.parts.as_deref_mut() {
                    place.many = None;
                }
                self.dispatch("parts.open", &serde_json::json!({ "part": one }))
            }
            _ => self.open_many(chosen),
        }
    }

    /// The sheet of many of the parts `chosen`.
    fn open_many(&mut self, chosen: Vec<String>) -> Outcome {
        let mut sheets = Vec::new();
        for c in &chosen {
            let Some(sheet) = self.sheet_named(c) else {
                return Outcome::refused(key!("reason-part-edit-target"));
            };
            let lives = self.parts_session().map_or(Lives::New, |s| s.sheet_lives(&sheet));
            sheets.push((sheet, lives));
        }
        let plain: Vec<PartSheet> = sheets.iter().map(|(s, _)| s.clone()).collect();
        let fields = ew_api::parts_many::shared_fields(&plain);
        if let Some(place) = self.parts.as_deref_mut() {
            place.sheet = None;
            place.asking = None;
            place.many = Some(Box::new(ManySheet {
                parts: chosen,
                sheets,
                fields,
                changed: false,
                error: None,
            }));
        }
        Outcome::Done
    }

    /// `parts.field` on the sheet of many: the value goes to every part, and the shared field reads it; a text that
    /// does not read is said under the sheet, and none of the parts takes it.
    pub(super) fn many_field(&mut self, field: &str, value: &str) -> Outcome {
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let session = &*session;
        let Some(many) = place.many.as_deref_mut() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let not_his = many.not_his();
        if !not_his.is_empty() {
            return Outcome::Refused {
                reason: strings::keyed(
                    key!("reason-parts-many-not-yours"),
                    vec![ew_api::TextArg::Plain(not_his.join(", "))],
                ),
            };
        }
        if !many.fields.iter().any(|(f, _)| f == field) {
            return failed(key!("error-part-field"), &[field]);
        }
        let mut sheets: Vec<PartSheet> = many.sheets.iter().map(|(s, _)| s.clone()).collect();
        for s in &mut sheets {
            match session.set_draft_field(s, field, value) {
                Ok(()) => {}
                Err(PartDraftError::Field(m)) => {
                    let words = strings::format(
                        key!("part-form-message"),
                        &[
                            &many.words(field),
                            &super::part_dialog::message_text(strings::Key::run_time(m.key), &m.args),
                        ],
                    );
                    many.error = Some(words.clone());
                    return Outcome::Failed { message: words };
                }
                Err(e) => return draft_refusal(e),
            }
        }
        for ((s, _), new) in many.sheets.iter_mut().zip(sheets) {
            *s = new;
        }
        let plain: Vec<PartSheet> = many.sheets.iter().map(|(s, _)| s.clone()).collect();
        many.fields = ew_api::parts_many::shared_fields(&plain);
        many.changed = true;
        many.error = None;
        Outcome::Done
    }

    /// Save of the sheet of many (Save, or Keep in my parts and save where parts only in this project are among them):
    /// one Save of them all ([`ShellModel::save_own`]): the parts of My parts as their next versions, the parts only
    /// in this project kept in My parts with the values typed, and the parts only in this project that any of them
    /// names kept once with them. A part that is not his is refused in words that name it.
    pub(super) fn save_many(&mut self) -> Outcome {
        let Some(many) = self.many_sheet() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let not_his = many.not_his();
        if !not_his.is_empty() {
            return Outcome::Refused {
                reason: strings::keyed(
                    key!("reason-parts-many-not-yours"),
                    vec![ew_api::TextArg::Plain(not_his.join(", "))],
                ),
            };
        }
        let sheets: Vec<PartSheet> = many.sheets.iter().map(|(s, _)| s.clone()).collect();
        let title = strings::format_count(crate::count_key!("parts-many-title"), sheets.len());
        let said = strings::format_count(crate::count_key!("parts-many-saved"), sheets.len());
        match self.save_own(&sheets, &title, said) {
            Ok(out) => {
                // Saved now; else the card waits, and the values stay typed until its Apply.
                if self.parts_save.is_none()
                    && let Some(m) = self.parts.as_deref_mut().and_then(|p| p.many.as_deref_mut())
                {
                    m.changed = false;
                }
                out
            }
            Err((i, e)) => {
                let number = sheets
                    .get(i)
                    .map(|s| s.text("part_number").to_owned())
                    .unwrap_or_default();
                let out = draft_refusal(e);
                let why = match &out {
                    Outcome::Refused { reason } => strings::render(reason),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                };
                let message = strings::format(key!("parts-many-refused"), &[&number, &why]);
                if let Some(m) = self.parts.as_deref_mut().and_then(|p| p.many.as_deref_mut()) {
                    m.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
        }
    }

    /// One Save of parts of his own, one or several (doc 07 §3.1): planned first ([`ew_api::Session::own_parts_save`]),
    /// nothing written. Where the project holds them, the card of what changes opens, naming the parts kept with them,
    /// and its Apply writes My parts and commits the one step ([`ShellModel::apply_own_save`]); its Cancel writes
    /// nothing. Else his Save is the yes: My parts is written, then the step that moves the kept parts' uses. `title`
    /// heads the card, `said` is the toast.
    pub(super) fn save_own(
        &mut self,
        sheets: &[PartSheet],
        title: &str,
        said: String,
    ) -> Result<Outcome, (usize, PartDraftError)> {
        let (Some(session), _) = self.parts_session_mut() else {
            return Ok(Outcome::Done);
        };
        let refs: Vec<&PartSheet> = sheets.iter().collect();
        let save = session.own_parts_save(&refs)?;
        let Some(plan) = save.plan.clone() else {
            session.write_own_save(&save).map_err(|e| (0, e))?;
            self.libraries_changed();
            let out = save
                .command
                .as_ref()
                .map_or(Outcome::Done, |c| self.run_commit(c));
            self.own_saved(&save, Some(said));
            return Ok(out);
        };
        let session = self.parts_session();
        let fields = plan
            .copies
            .iter()
            .flat_map(|c| card_fields(session, &c.fields))
            .collect();
        let devices = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        let card = PartCard {
            title: title.to_owned(),
            fields,
            devices,
            removed: plan.bom_removed,
            added: plan.bom_added,
            changed: plan.bom_changed,
            boxes: [
                self.box_words(&plan.boxes),
                self.symbol_words(&plan.symbols),
                self.own_symbol_words(&plan.own_symbols),
            ]
            .concat(),
            lost: plan.lost,
            kept: save.with.clone(),
            fork: false,
        };
        self.parts_save = Some(Box::new(save));
        Ok(self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-library-update"),
            CardOrigin::Part(Box::new(card)),
        ))
    }

    /// Apply of a part card: where its Save waits ([`ShellModel::save_own`]), My parts is written first, then the card's
    /// step is committed; a write that fails leaves the card and the project as they were.
    pub(crate) fn apply_own_save(&mut self, card: crate::properties::Card) -> Outcome {
        let save = self.parts_save.take().filter(|s| {
            s.plan
                .as_ref()
                .is_some_and(|p| p.command.params == card.edit.params)
        });
        let Some(save) = save else {
            return self.apply_bulk_card(card);
        };
        if let (Some(session), _) = self.parts_session_mut()
            && let Err(e) = session.write_own_save(&save)
        {
            self.props.card = Some(card);
            self.parts_save = Some(save);
            return draft_refusal(e);
        }
        self.libraries_changed();
        let out = self.apply_bulk_card(card);
        if out == Outcome::Done {
            self.own_saved(&save, None);
        }
        out
    }

    /// After a Save of his own parts is written and committed: the toast `said` with the parts kept with them, the
    /// list again, and the sheet of several over the kept parts.
    fn own_saved(&mut self, save: &ew_api::parts_many::OwnSave, said: Option<String>) {
        if let Some(place) = self.parts.as_deref_mut() {
            if let Some(open) = place.sheet.as_mut() {
                open.changed = false;
            }
            if let Some(m) = place.many.as_deref_mut() {
                m.changed = false;
            }
        }
        let kept = (!save.with.is_empty())
            .then(|| strings::format(key!("parts-saved-kept"), &[&save.with.join(", ")]));
        let text = match (said, kept) {
            (Some(s), Some(k)) => Some(format!("{s} {k}")),
            (s, k) => s.or(k),
        };
        if let Some(text) = text {
            self.notify(text, false);
        }
        if let Some(m) = self.parts.as_deref_mut().and_then(|p| p.many.as_deref_mut()) {
            for p in &mut m.parts {
                if let Some((_, to)) = save.moved.iter().find(|(from, _)| *p == format!("id:{from}")) {
                    *p = format!("id:{to}");
                }
            }
        }
        if self.many_sheet().is_some() {
            let _ = self.reopen_many();
        }
        self.parts_refresh();
    }
    /// The sheet of many read again from where the parts live now (after a Save, a Discard).
    pub(super) fn reopen_many(&mut self) -> Outcome {
        let Some(parts) = self.many_sheet().map(|m| m.parts.clone()) else {
            return Outcome::Done;
        };
        self.open_many(parts)
    }

    /// `parts.export {path?}` (Export as a table… of Parts, doc 07 §5.9): the selected parts, else the open one, as a
    /// table of their declared fields in the window's words, a CSV or a workbook by the file's extension; without
    /// `path` the window's file dialog asks. Import… reads it back as the same.
    pub(crate) fn act_parts_export(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            // The native save dialog asks about an existing file itself, so its answer carries `replace`.
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                return self.need_path("parts.export", Prompt::SaveTable, "path", &rest);
            }
        };
        // The parts as they are saved: what was typed and not saved is not in the file, and the toast says so.
        let (named, typed): (Vec<String>, bool) = match (
            self.many_sheet(),
            self.parts.as_deref().and_then(|p| p.sheet.as_ref()),
        ) {
            (Some(m), _) => (m.parts.clone(), m.changed),
            (None, Some(open)) if !open.sheet.new => {
                (self.sheet_param(&open.sheet).into_iter().collect(), open.changed)
            }
            _ => return Outcome::refused(key!("reason-no-part-open")),
        };
        let Some(sheets) = named
            .iter()
            .map(|n| self.sheet_named(n))
            .collect::<Option<Vec<PartSheet>>>()
            .filter(|s| !s.is_empty())
        else {
            return Outcome::refused(key!("reason-part-edit-target"));
        };
        if path.exists() && !flag(p, "replace") {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let columns = ew_api::parts_many::table_columns(&sheets);
        let header = |member: &str| {
            let plain = super::parts::PartsMap::member_text(member, "");
            match ew_api::part_fields::field(member) {
                // The identity's fields by their word alone ("Part number"), the others with their group.
                Some(f) if f.group == ew_api::part_fields::Group::Identity => {
                    strings::render(&ew_api::part_fields::word(f.word))
                }
                _ => plain,
            }
        };
        let table = Table {
            name: strings::text(key!("parts-title")).to_owned(),
            key: "parts".to_owned(),
            columns: columns
                .iter()
                .map(|(m, _)| TableColumn {
                    key: m.clone(),
                    label: header(m),
                    cell: ew_api::output::CellType::Text,
                    editable: false,
                })
                .collect(),
            rows: (0..sheets.len())
                .map(|i| TableRow {
                    id: Some(sheets[i].text("part_number").to_owned()),
                    heading: false,
                    cells: columns
                        .iter()
                        .map(|(_, t)| TableCell::Text(t[i].clone()))
                        .collect(),
                })
                .collect(),
        };
        let xlsx = path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("xlsx"));
        let bytes = if xlsx {
            // No creation date, as the navigators' tables without `SOURCE_DATE_EPOCH`.
            let Some(meta) = self.parts_session().map(|s| s.workbook_meta(None)) else {
                return Outcome::refused(key!("reason-no-parts-place"));
            };
            match ew_api::output::xlsx(std::slice::from_ref(&table), &meta) {
                Ok(b) => b,
                Err(e) => return failed(key!("error-export"), &[&e.to_string()]),
            }
        } else {
            ew_api::output::table_csv(&table, &CsvOptions::preset(Preset::Rfc4180)).into_bytes()
        };
        // All or nothing: the file is written beside and then renamed over the target.
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let part = crate::jobs::sibling_of(&path, &name);
        let written = std::fs::write(&part, bytes).and_then(|()| std::fs::rename(&part, &path));
        if let Err(e) = written {
            let _ = std::fs::remove_file(&part);
            return failed(key!("error-export"), &[&e.to_string()]);
        }
        let said = if typed {
            crate::count_key!("parts-exported-saved")
        } else {
            crate::count_key!("parts-exported")
        };
        self.notify(strings::format_counted(said, sheets.len(), &[&name]), false);
        Outcome::Done
    }
}
