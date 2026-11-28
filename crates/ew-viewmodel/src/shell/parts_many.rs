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

    /// Save of the sheet of many: every part changed is saved as its next version in My parts; where the project holds
    /// them, the card of the one update names how many parts and their changed fields and the devices, before its
    /// Apply. The parts only in this project among them are kept in My parts first (Keep in my parts and save, one
    /// undo step of its own), and the values typed go to the kept parts. A part that is not his is refused in words
    /// that name it.
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
        if many.sheets.iter().any(|(_, l)| *l == Lives::OnlyHere) {
            let out = self.keep_many();
            if out != Outcome::Done {
                return out;
            }
        }
        let Some(many) = self.many_sheet() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let sheets: Vec<PartSheet> = many.sheets.iter().map(|(s, _)| s.clone()).collect();
        let (Some(session), _) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let refs: Vec<&PartSheet> = sheets.iter().collect();
        let (count, plan) = match session.own_parts_update(&refs) {
            Ok(p) => p,
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
                return Outcome::Failed { message };
            }
        };
        if let Some(m) = self.parts.as_deref_mut().and_then(|p| p.many.as_deref_mut()) {
            m.changed = false;
        }
        self.libraries_changed();
        let Some(plan) = plan else {
            self.notify(
                strings::format_count(crate::count_key!("parts-many-saved"), count),
                false,
            );
            return self.reopen_many();
        };
        let devices = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        let session = self.parts_session();
        let fields = plan
            .copies
            .iter()
            .flat_map(|c| card_fields(session, &c.fields))
            .collect();
        let card = PartCard {
            title: strings::format_count(crate::count_key!("parts-many-title"), count),
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
            fork: false,
        };
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-library-update"),
            CardOrigin::Part(Box::new(card)),
        )
    }

    /// The parts only in this project of the sheet of many kept in My parts (Keep in my parts, one step), and the sheet
    /// of many again over the kept parts with the values that were typed into all of them.
    fn keep_many(&mut self) -> Outcome {
        let Some(many) = self.many_sheet() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let typed: Vec<(String, String)> = (many.fields.iter())
            .filter_map(|(f, v)| Some((f.clone(), v.clone()?)))
            .collect();
        let here: Vec<ew_api::output::Uuid> = (many.sheets.iter())
            .filter(|(_, l)| *l == Lives::OnlyHere)
            .map(|(s, _)| s.copy)
            .collect();
        let parts = many.parts.clone();
        let (Some(session), _) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let kept = match session.keep_in_my_parts(&here) {
            Ok(k) => k,
            Err(e) => {
                let out = draft_refusal(e);
                if let (Outcome::Failed { message }, Some(m)) = (
                    &out,
                    self.parts.as_deref_mut().and_then(|p| p.many.as_deref_mut()),
                ) {
                    m.error = Some(message.clone());
                }
                return out;
            }
        };
        let out = self.run_commit(&kept.command);
        if out != Outcome::Done {
            return out;
        }
        self.libraries_changed();
        // Each part kept is named by its kept copy now.
        let p = &kept.command.params;
        let moved: Vec<(String, String)> = std::iter::once((&p["from"], &p["to"]))
            .chain((p["also"].as_array().into_iter().flatten()).map(|k| (&k["from"], &k["to"])))
            .filter_map(|(f, t)| Some((format!("id:{}", f.as_str()?), format!("id:{}", t.as_str()?))))
            .collect();
        let parts = parts
            .into_iter()
            .map(|c| moved.iter().find(|(f, _)| *f == c).map_or(c, |(_, t)| t.clone()))
            .collect();
        let out = self.open_many(parts);
        if out != Outcome::Done {
            return out;
        }
        for (field, value) in typed {
            let out = self.many_field(&field, &value);
            if out != Outcome::Done {
                return out;
            }
        }
        Outcome::Done
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
