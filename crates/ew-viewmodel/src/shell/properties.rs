// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid in the shell: the sheet fetched once per key, the row
//! cursor, the edit buffer, the commit path, the card, the toast and `select.set`.
//!
//! **The sheet** is fetched at the end of a dispatch when the pane is in the layout and its key (project session,
//! revision, the records) changed: a selection change, a focused-page change with an empty selection, or a new
//! revision makes exactly one `Session::property_sheet` call; row moves, category toggles, typing, choices and popups
//! make none. With the pane closed there is no sheet and no call.
//!
//! **The commit path**: an edit compiles into exactly one command (`fields.set` or `project.settings.set`),
//! refused unless its exposure is `Edit`. It is previewed once (`Session::preview_with`); the edit policy decides
//! on the preview: a direct edit commits the kept preview (the window: `Session::commit`; headless: a one-line command
//! log through `Session::run_script`, as `command.run`), any other shows the card. A stale card is never re-previewed
//! with its old parameters: Apply checks the edited member against the base value and refuses
//! `reason-changed-meanwhile` when it moved, else rebuilds the edit on the current values and shows a fresh card.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_api::{AnyCommand, CmdError};
use ew_dl::{Destination, EntityRef};
use serde_json::{Map, Value};

use super::navigation::{Focus, bump};
use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::navigators::NavKind;
use crate::properties::{
    CardOrigin, PropRow, PropertiesState, PropertySheet, RowAction, RowValue, SheetKey, SheetSection,
    filter_shown, find_row, is_derived, visible_rows,
};
use crate::strings;
use crate::strings::Key;

mod edit;

mod commit;

mod rows;

/// The message of a command refusal as the grid shows it: a parameter path `/targets/<i>` names the target by
/// its label (`label(i)`, "on -101F1: …").
fn refusal_text(e: &CmdError, label: &dyn Fn(usize) -> String) -> String {
    match e {
        // The reason in the window's language.
        CmdError::InvalidParams { path, text, .. } => {
            let message = super::tools::rejected_words("", Some(&**text));
            match path
                .strip_prefix("/targets/")
                .and_then(|i| i.parse::<usize>().ok())
            {
                Some(i) => strings::format(key!("props-refused-on"), &[&label(i), &message]),
                None => message,
            }
        }
        CmdError::Rejected { text, .. } => super::tools::rejected_words("", Some(&**text)),
        CmdError::NotFound { .. } => strings::text(key!("props-gone")).to_owned(),
        // A prevent rule, a model or history error in the window's language.
        other => super::tools::command_error_words(other),
    }
}

/// `text` with every hyphenated UUID replaced by `name(uuid)` (no UUID reaches the transcript).
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

/// The key of a terminal's Group row and of its Printed label row.
const GROUP_ROW: &str = "device/derived.group";
/// A panel page's Surface view row: its choices are the surfaces by ID.
const SURFACE_ROW: &str = "page/derived.surface";
const PRINTED_LABEL_ROW: &str = "device/derived.printed_label";
/// The key of a device's Tag row.
const TAG_ROW: &str = "device/derived.tag";
// A page's Connection grid row, a choice of the grids New page offers, in their words.

/// The choices of the Connection grid row: each grid of `PAGE_GRIDS` in its words, with its value.
fn grid_choices() -> Vec<(String, Value)> {
    super::creation::PAGE_GRIDS
        .iter()
        .filter_map(|w| Some((strings::choice_text(w), super::creation::page_grid_value(w)?)))
        .collect()
}

/// The choices of a page's and a report block's Form row: the project's forms, each by its name in the window's
/// language (doc 09 §3.8).
fn form_choices(session: &ew_api::Session) -> Vec<(ew_api::output::Uuid, String)> {
    session
        .frame_forms()
        .iter()
        .map(|(id, name)| (*id, super::text_words(name)))
        .collect()
}

/// The grid row and value of a tag typed for a device whose tag on show is `shown` (letter `letter`, counter
/// `counter`) when the typed text changes exactly one of the two and keeps the rest: `-102F1` → `-102K1` is the Letter
/// row with `K`, `-X1031` → `-X1032` the Counter row with `2` (the strip format `{letter}{page}{counter}`). `None` for
/// any other text (the same tag, both changed, another prefix), which the Tag row takes.
fn tag_edit_of(shown: &str, letter: &str, counter: &str, typed: &str) -> Option<(&'static str, String)> {
    if letter.is_empty() || counter.is_empty() {
        return None;
    }
    let head = shown.strip_suffix(counter)?;
    let at = head.rfind(letter)?;
    let (before, after) = (&head[..at], &head[at + letter.len()..]);
    let rest = typed.trim().strip_prefix(before)?;
    let typed_letter: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
    let typed_counter = rest[typed_letter.len()..].strip_prefix(after)?;
    if typed_letter.is_empty()
        || typed_counter.is_empty()
        || !typed_counter.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    match (typed_letter != letter, typed_counter != counter) {
        (true, false) => Some(("device/tag.letter", typed_letter)),
        (false, true) => Some(("device/tag.counter", typed_counter.to_owned())),
        _ => None,
    }
}

/// The sentence of a strip edit's refusal (the Group row's error line).
fn message_of(result: &Result<AnyCommand, ew_api::output::StripEditError>) -> String {
    match result {
        Err(ew_api::output::StripEditError::Invalid(m)) => super::tools::rejected_words("", Some(m)),
        Err(ew_api::output::StripEditError::Refused(k)) => strings::text(Key::run_time(k)).to_owned(),
        Err(ew_api::output::StripEditError::Ambiguous(c)) => {
            c.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join("; ")
        }
        Ok(_) => String::new(),
    }
}

impl ShellModel {
    /// A command refusal as the grid shows it: [`refusal_text`] with the target named by its label (the header's title
    /// for one target, else the title of that record's own sheet: one `Session::property_sheet` call, on a refusal
    /// only), page IDs as folios and other IDs left out.
    pub(super) fn refusal(&self, e: &CmdError, section: Option<&SheetSection>) -> String {
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
                _ => strings::format(key!("props-target-n"), &[&(i + 1).to_string()]),
            }
        };
        replace_uuids(&refusal_text(e, &label), &|u| {
            rows.iter().find(|r| r.id.to_string() == u).map_or_else(
                || strings::text(key!("props-a-record")).to_owned(),
                |r| r.folio.clone(),
            )
        })
    }

    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The property grid's state.
    pub fn properties(&self) -> &PropertiesState {
        &self.props
    }

    /// The form a bus port's field takes, said under its editor: for `bus_port.address`
    /// the address form of the port's bus with an example (`Session::bus_address_form`), for `bus_port.termination`
    /// its words; `None` for another field or a function without a bus port.
    fn port_field_hint(&self, function: ew_api::output::Uuid, field: &str) -> Option<String> {
        match field {
            "bus_port.address" => {
                let key = self.project_session()?.bus_address_form(function)?;
                Some(strings::text(Key::run_time(key)).to_owned())
            }
            "bus_port.termination" => Some(strings::text(key!("hint-termination")).to_owned()),
            _ => None,
        }
    }

    /// What the open Properties editor says under it: the form of a bus port's address or termination (one port
    /// selected); `None` for another row.
    pub fn property_edit_hint(&self) -> Option<String> {
        let edit = self.props.edit()?;
        let field = edit.row.strip_prefix("function/")?;
        let (section, _) = crate::properties::find_row(self.props.sheet()?, &edit.row)?;
        match section.targets.as_slice() {
            [function] => self.port_field_hint(*function, field),
            _ => None,
        }
    }

    /// What the edited cell of navigator `kind`'s table says under it: in the Bus table the form of the port's
    /// Address or Termination, then, with more than one row marked, that Ctrl+Enter sets them all
    /// (`navigators::table::commit_all_hint`), a line each; `None` when neither applies.
    pub fn table_edit_hint(&self, kind: NavKind) -> Option<String> {
        let nav = self.nav(kind)?;
        let form = (kind == NavKind::Bus)
            .then(|| {
                let edit = nav.table_mode()?.edit()?;
                let field = match edit.column.as_str() {
                    "address" => "bus_port.address",
                    "termination" => "bus_port.termination",
                    _ => return None,
                };
                let (_, node) = nav.node_of(&edit.row)?;
                self.port_field_hint(*node.records.first()?, field)
            })
            .flatten();
        let all = crate::navigators::table::commit_all_hint(nav);
        match (form, all) {
            (Some(f), Some(a)) => Some(format!("{f}\n{a}")),
            (f, a) => f.or(a),
        }
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

    /// The collapsed categories (`device/identity`), from the settings file (`properties.collapsed`). A section's More
    /// group is closed until a person opens it: its key in the settings means it is open.
    pub fn properties_collapsed(&self) -> BTreeSet<String> {
        let stored = &self.settings.properties_collapsed;
        let mut out: BTreeSet<String> = stored
            .iter()
            .filter(|k| !crate::properties::is_more_key(k))
            .cloned()
            .collect();
        for s in self.props.sheet.iter().flat_map(|sheet| &sheet.sections) {
            let more = crate::properties::group_key(&s.kind, crate::properties::RowGroup::More);
            if !stored.contains(&more) {
                out.insert(more);
            }
        }
        out
    }

    /// Opens or closes the group `key` in the settings (a More group's key is stored while it is open); whether that
    /// changed them.
    fn set_group_open(&mut self, key: String, open: bool) -> bool {
        let stored = &mut self.settings.properties_collapsed;
        let keep = open == crate::properties::is_more_key(&key);
        if stored.contains(&key) == keep {
            return false;
        }
        if keep {
            stored.push(key);
            stored.sort();
        } else {
            stored.retain(|k| *k != key);
        }
        true
    }

    /// Whether the grid shows the Advanced rows (`[properties] show_advanced`; `props.show_advanced`).
    pub fn properties_show_advanced(&self) -> bool {
        self.settings.properties_show_advanced
    }

    /// `props.show_advanced {on?}`: shows or hides the Advanced rows (else it toggles), remembered in the settings file.
    pub(crate) fn act_props_show_advanced(&mut self, p: &Map<String, Value>) -> Outcome {
        let on = match p.get("on") {
            None => !self.settings.properties_show_advanced,
            Some(v) => match v.as_bool() {
                Some(b) => b,
                None => return failed(key!("error-bad-param"), &["on"]),
            },
        };
        if on != self.settings.properties_show_advanced {
            self.settings.properties_show_advanced = on;
            self.save_settings();
        }
        Outcome::Done
    }

    /// Doc 16a §6.8: whether the grid shows its filter field, the open categories holding more than twelve
    /// rows.
    pub fn properties_filter_shown(&self) -> bool {
        self.props.sheet().is_some_and(|sheet| {
            filter_shown(
                sheet,
                &self.properties_collapsed(),
                self.settings.properties_show_advanced,
            )
        })
    }

    /// The filter in force: the field's text while the field is on show, else empty (a filter whose field is not on
    /// show hides nothing).
    pub fn properties_filter(&self) -> &str {
        if self.properties_filter_shown() {
            self.props.filter.trim()
        } else {
            ""
        }
    }

    /// `props.filter {text}`: sets the text of the grid's filter field; the rows on show are those whose label
    /// or value contains it, case-folded, while the field is on show.
    pub(crate) fn act_props_filter(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = match string_param(p, "text") {
            Err(e) => return e,
            Ok(None) => return failed(key!("error-missing-param"), &["text"]),
            Ok(Some(t)) => t.to_owned(),
        };
        self.props.filter = text;
        self.props.popup = None;
        Outcome::Done
    }

    /// The focused row: the row the cursor was moved to while it is on show, else the first row on show.
    pub fn properties_row(&self) -> Option<String> {
        let sheet = self.props.sheet()?;
        let collapsed = self.properties_collapsed();
        let rows = visible_rows(
            sheet,
            &collapsed,
            self.settings.properties_show_advanced,
            self.properties_filter(),
        );
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
        if self.props.project_mode == Some((generation, self.current_page())) {
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
            frame: self.frame_choice().cloned(),
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
            self.props.tag_proposal = None;
            self.props.choices = None;
            self.props.popup = None;
            // The grid's card goes; a replace card is the find pane's and does not follow the selection.
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
        let kinds = |s: &PropertySheet| s.sections.iter().map(|x| x.kind.clone()).collect::<Vec<_>>();
        let before = self.props.sheet.as_deref().map(kinds);
        let sheet = match key.frame.as_ref().and_then(|c| self.frame_sheet(c)) {
            Some(frame) => frame,
            None => self.fetch_sheet(&key.ids),
        };
        // A filter typed for one kind of thing does not hide the rows of another ("grid" typed on a page hid a
        // breaker's rows): it clears when the selection changes kind.
        if !same_records && before.is_some_and(|b| b != kinds(&sheet)) {
            self.props.filter.clear();
        }
        self.props.sheet = Some(Arc::new(sheet));
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
        crate::properties::grid_in_words(&mut sheet);
        crate::properties::style_in_words(&mut sheet);
        crate::properties::ends_in_words(&mut sheet, |id| p.session.selection_words(id));
        crate::properties::report_columns_in_words(&mut sheet, |id| {
            p.session.report_column_words(id, strings::rule_language())
        });
        crate::properties::forms_in_words(
            &mut sheet,
            || form_choices(&p.session),
            |page| p.session.page_default_frame(page),
        );
        crate::properties::enums_in_words(&mut sheet);
        crate::properties::command_choices_in_words(&mut sheet);
        crate::properties::surfaces_in_words(&mut sheet, &uuids, |id| {
            p.session
                .layout_words(id.parse().ok()?)
                .map(|w| strings::render(&w))
        });
        crate::properties::report_source_in_words(&mut sheet);
        self.wire_numbers_in_row(&mut sheet);
        crate::properties::values_in_words(&mut sheet);
        self.made_from_in_words(&mut sheet);
        Self::people_in_rows(&mut sheet);
        self.own_fields_in_rows(&mut sheet);
        if sheet.sections.iter().any(|s| s.kind == "project") {
            self.lists_frame_in_row(&mut sheet, &form_choices(&p.session));
        }
        // The Part function row's kind in the window's language: each target's words, by the English its text prints.
        for section in &mut sheet.sections {
            let kinds: Vec<(String, String)> = (section.targets.iter())
                .filter_map(|id| p.session.part_function_words(*id))
                .map(|t| (t.to_string(), strings::render(&t)))
                .collect();
            let words = |t: &mut String| {
                if let Some((_, w)) = kinds.iter().find(|(english, _)| english == t) {
                    *t = w.clone();
                }
            };
            for row in section.rows.iter_mut().filter(|r| r.path == "template_entry") {
                match &mut row.value {
                    RowValue::One { text, .. } => words(text),
                    RowValue::Varies { texts, .. } => texts.iter_mut().for_each(&words),
                    RowValue::Absent => {}
                }
            }
        }
        crate::properties::cps_in_words(&mut sheet);
        crate::properties::solenoids_in_words(&mut sheet);
        crate::properties::ports_in_words(&mut sheet);
        crate::properties::connection_kind_in_words(&mut sheet);
        self.positions_in_words(&mut sheet, p.session.decimal_comma());
        Self::valve_positions_in_words(&mut sheet, &p.session);
        crate::properties::header_in_words(&mut sheet);
        // The header's finding chips in the UI language, as the Messages detail renders
        // them, for the online report's finding of the chip's rule and message.
        let lang = strings::rule_language();
        if lang != "en-GB" && !sheet.header.findings.is_empty() {
            let report = p
                .session
                .check(p.session.online_scheme(), ew_api::checks::CheckMode::Online);
            for chip in &mut sheet.header.findings {
                if let Some(f) = report
                    .findings
                    .iter()
                    .find(|f| f.rule == chip.rule && f.message() == chip.message)
                {
                    chip.message = ew_api::checks::messages::render(f, lang);
                }
            }
        }
        // A read-only project's rows offer nothing.
        if sheet.read_only.is_some() {
            for s in &mut sheet.sections {
                for r in &mut s.rows {
                    r.actions.clear();
                }
            }
        }
        sheet
    }

    /// The project's Made from template row by the template's name in the list (a dash must not stand for a
    /// project made from a template), else in words that it is no template of the list; the ID and the generation the
    /// project records under it say nothing to a person and are left out.
    fn made_from_in_words(&self, sheet: &mut PropertySheet) {
        for section in sheet.sections.iter_mut().filter(|s| s.kind == "project") {
            let Some(row) = section.rows.iter_mut().find(|r| r.path == "basic_project") else {
                continue;
            };
            let RowValue::One { json, text } = &mut row.value else {
                continue;
            };
            let Some(id) = json.get("id").and_then(Value::as_str).map(str::to_owned) else {
                continue;
            };
            *text = self
                .settings
                .templates
                .iter()
                .find(|t| {
                    ew_api::template_summary(std::path::Path::new(&t.dir))
                        .is_ok_and(|s| s.id.to_string() == id)
                })
                .map_or_else(
                    || strings::text(key!("props-template-not-listed")).to_owned(),
                    |t| t.name.clone(),
                );
            row.ty = crate::properties::FieldType::Text;
            section.rows.retain(|r| !r.path.starts_with("basic_project."));
        }
    }

    /// A representation's position reads in millimetres, with its grid cell when the focused
    /// canvas shows its page (`102-C5 · x 18.5 y 74.1 mm`), never in nanometres; one decimal, as the
    /// status bar's, with the project's decimal separator (`comma`).
    fn positions_in_words(&self, sheet: &mut PropertySheet, comma: bool) {
        let page = self.canvas().map(|c| c.page());
        for section in sheet.sections.iter_mut().filter(|s| s.kind == "representation") {
            let rep = section.targets.first().copied();
            for row in section.rows.iter_mut().filter(|r| r.key == "representation/pos") {
                let RowValue::One { json, text } = &mut row.value else {
                    continue;
                };
                let Some([x, y]) = json
                    .as_array()
                    .and_then(|a| Some([a.first()?.as_i64()?, a.get(1)?.as_i64()?]))
                else {
                    continue;
                };
                let mm = |nm: i64| {
                    let text = crate::canvas::mm1(nm);
                    if comma { text.replace('.', ",") } else { text }
                };
                let (x_mm, y_mm) = (mm(x), mm(y));
                let cell = page
                    .filter(|p| p.tables.reps.iter().any(|r| Some(r.id) == rep))
                    .and_then(|p| p.cell_text(p.snapper.page_to_sheet((x, y))));
                *text = match cell {
                    Some(cell) => strings::format(key!("props-position-cell"), &[&cell, &x_mm, &y_mm]),
                    None => strings::format(key!("props-position"), &[&x_mm, &y_mm]),
                };
            }
        }
    }

    /// A valve terminal's Valve positions row in the window's words: `1 -K1-Q1 · 2 blanking plate · 3 empty`.
    fn valve_positions_in_words(sheet: &mut PropertySheet, session: &ew_api::Session) {
        for section in sheet.sections.iter_mut().filter(|s| s.kind == "device") {
            let Some(terminal) = section.targets.first().copied() else {
                continue;
            };
            for row in section
                .rows
                .iter_mut()
                .filter(|r| r.key == "device/derived.valve_positions")
            {
                if let RowValue::One { text, .. } = &mut row.value {
                    *text = session
                        .terminal_positions(terminal)
                        .iter()
                        .map(|(n, p)| format!("{n} {}", super::station::rack_position_text(p)))
                        .collect::<Vec<_>>()
                        .join(" · ");
                }
            }
        }
    }

    /// The row named by `row` in `p`, else the focused row.
    pub(super) fn props_row_param(&self, p: &Map<String, Value>) -> Result<String, Outcome> {
        match string_param(p, "row")? {
            Some(r) => {
                let sheet = self
                    .props
                    .sheet()
                    .ok_or(Outcome::refused(key!("reason-no-sheet")))?;
                if find_row(sheet, r).is_none() {
                    return Err(failed(key!("error-no-row"), &[r]));
                }
                Ok(r.to_owned())
            }
            None => self
                .properties_row()
                .ok_or(Outcome::refused(key!("reason-no-row"))),
        }
    }

    /// The section and row of `key` in the current sheet, cloned.
    pub(super) fn props_find(&self, key: &str) -> Option<(SheetSection, PropRow)> {
        let sheet = self.props.sheet()?;
        find_row(sheet, key).map(|(s, r)| (s.clone(), r.clone()))
    }

    /// Why a row cannot take `action` now, as the outcome that refuses it: `None` when it can. A read-only
    /// project refuses with `reason-read-only`; in an editable project a derived row is computed
    /// (`reason-row-computed`), a human-only row keeps `reason-human-only`, a row with another read-only reason reads
    /// that reason in a sentence (`crate::properties::refusal_text`, a formatted refusal), and an editable
    /// row that does not
    /// offer the action reads `reason-action-not-offered`.
    fn props_refusal(&self, section: &SheetSection, row: &PropRow, action: RowAction) -> Option<Outcome> {
        let sheet = self.props.sheet()?;
        let refused = |reason_key: Key<'static>| {
            Some(Outcome::Refused {
                reason: reason_key.into(),
            })
        };
        if sheet.read_only.is_some() || !self.is_writable() {
            return refused(key!("reason-read-only"));
        }
        if row.actions.contains(&action) {
            return None;
        }
        if is_derived(row) {
            return refused(key!("reason-row-computed"));
        }
        match crate::properties::refusal_text(sheet, section, row) {
            Some(_)
                if matches!(
                    row.route,
                    crate::properties::EditRoute::ReadOnly(
                        crate::properties::FieldReadOnly::HumanOnly { .. }
                    )
                ) =>
            {
                refused(key!("reason-human-only"))
            }
            Some(message) => Some(Outcome::Failed { message }),
            None => refused(key!("reason-action-not-offered")),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // View actions

    /// `props.show` (View ▸ Properties): toggles the pane, or sets it with `on`.
    pub(crate) fn act_props_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let shown = self
            .panes
            .layout
            .panes()
            .contains(&crate::layout::PaneKind::Properties);
        let want = p.get("on").and_then(Value::as_bool).unwrap_or(!shown);
        if want != shown {
            let layout = if want {
                self.panes.layout.with_properties()
            } else {
                self.panes.layout.without(crate::layout::PaneKind::Properties)
            };
            self.replace_layout(layout);
            if !want && self.focus == Some(Focus::Properties) {
                self.focus = self.area.as_ref().map(|a| Focus::Canvas(a.focused));
            }
        }
        Outcome::Done
    }

    /// `props.focus` (E, Alt+Enter on the canvas): the pane gets the focus (shown first when closed).
    pub(crate) fn act_props_focus(&mut self) -> Outcome {
        if !self
            .panes
            .layout
            .panes()
            .contains(&crate::layout::PaneKind::Properties)
        {
            let _ = self.act_props_show(&Map::new());
        }
        self.focus = Some(Focus::Properties);
        Outcome::Done
    }

    /// `props.project` (Project properties): the project record until the selection changes, `select.set` sets it
    /// (`{"ids": []}` too), or another page shows; the page's own rows show again then.
    pub(crate) fn act_props_project(&mut self) -> Outcome {
        self.props.project_mode = Some((self.tools.selection.generation, self.current_page()));
        self.act_props_focus()
    }

    /// `select.set`: `ids` (UUID texts) or `refs` (command-log references); an empty list clears; one unresolvable
    /// entry refuses the whole action with the resolver's message.
    pub(crate) fn act_select_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let project = or_return!(crate::shell::gated(&self.project));
        let mut items: Vec<EntityRef> = Vec::new();
        match (p.get("ids"), p.get("refs")) {
            (Some(Value::Array(ids)), None) => {
                for v in ids {
                    let Some(text) = v.as_str() else {
                        return failed(key!("error-bad-param"), &["ids"]);
                    };
                    match project.session.resolve_reference(&format!("id:{text}")) {
                        Ok(u) => items.push(EntityRef(u)),
                        Err(why) => {
                            return Outcome::Failed {
                                message: super::tools::rejected_words("", Some(&why)),
                            };
                        }
                    }
                }
            }
            (None, Some(Value::Array(refs))) => {
                for v in refs {
                    let Some(text) = v.as_str() else {
                        return failed(key!("error-bad-param"), &["refs"]);
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
                        Err(why) => {
                            return Outcome::Failed {
                                message: super::tools::rejected_words("", Some(&why)),
                            };
                        }
                    }
                }
            }
            (None, None) => return failed(key!("error-missing-param"), &["ids"]),
            _ => return failed(key!("error-bad-param"), &["ids"]),
        }
        let change = crate::tools::SelectionChange::Replace(items);
        if self.tools.selection.apply(&change) {
            self.announce_selection();
        }
        // A selection set, even the empty one it was, ends the project's sheet (`props.project`).
        self.props.project_mode = None;
        Outcome::Done
    }

    /// `edit.select_all` (Ctrl+A on the canvas, doc 16 §4.6): every object the page shown draws, as a rubber band
    /// around the whole page would take them.
    pub(crate) fn act_select_all(&mut self) -> Outcome {
        let Some(c) = self.canvas() else {
            return Outcome::refused(key!("reason-no-page"));
        };
        let entities = &c.page().entities;
        // In the band's reading order, so that a replay lists them as the window did.
        let items = match entities.iter().map(|(_, r)| *r).reduce(|a, b| a.union(b)) {
            Some(all) => crate::tools::band_selection(entities, all, true),
            None => Vec::new(),
        };
        if self
            .tools
            .selection
            .apply(&crate::tools::SelectionChange::Replace(items))
        {
            self.announce_selection();
        }
        Outcome::Done
    }

    /// The drawn connection of the page shown whose ends the page text names `<a> -> <b>` (either way round; quotes
    /// around an end are optional): a wire whose drawn route another wire or a symbol's body covers
    /// cannot be clicked, so `select.set` takes `wire:<a> -> <b>` beside the session's references. No such wire, or
    /// several, fail with a message.
    fn wire_of_page(&self, ends: &str) -> Result<EntityRef, Outcome> {
        let unquote = |s: &str| s.trim().trim_matches('"').to_owned();
        let Some((a, b)) = ends.split_once("->").map(|(a, b)| (unquote(a), unquote(b))) else {
            return Err(failed(key!("error-wire-reference"), &[ends]));
        };
        let Some(c) = self.canvas() else {
            return Err(Outcome::refused(key!("reason-no-page")));
        };
        let hits: Vec<EntityRef> = crate::describe::page_wires(c.page())
            .into_iter()
            .filter(|(_, x, y)| (*x == a && *y == b) || (*x == b && *y == a))
            .map(|(e, _, _)| e)
            .collect();
        match hits.as_slice() {
            [one] => Ok(*one),
            [] => Err(failed(key!("error-wire-reference"), &[ends])),
            _ => Err(failed(key!("error-wire-ambiguous"), &[ends])),
        }
    }

    /// `props.row`: `step` (±n, clamped) or `to` (a row key, `first`, `last`).
    pub(crate) fn act_props_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sheet) = self.props.sheet.clone() else {
            return Outcome::refused(key!("reason-no-sheet"));
        };
        let collapsed = self.properties_collapsed();
        let filter = self.properties_filter().to_owned();
        let rows = visible_rows(
            &sheet,
            &collapsed,
            self.settings.properties_show_advanced,
            &filter,
        );
        if rows.is_empty() {
            return Outcome::refused(key!("reason-no-row"));
        }
        let here = self
            .properties_row()
            .and_then(|k| rows.iter().position(|r| r.row.key == k))
            .unwrap_or(0);
        let at = if let Some(step) = p.get("step") {
            let Some(step) = step.as_i64() else {
                return failed(key!("error-bad-param"), &["step"]);
            };
            (here as i64).saturating_add(step).clamp(0, rows.len() as i64 - 1) as usize
        } else {
            match string_param(p, "to") {
                Err(e) => return e,
                Ok(Some("first")) => 0,
                Ok(Some("last")) => rows.len() - 1,
                Ok(Some(key)) => match rows.iter().position(|r| r.row.key == key) {
                    Some(i) => i,
                    None => return failed(key!("error-no-row"), &[key]),
                },
                Ok(None) => return failed(key!("error-missing-param"), &["step"]),
            }
        };
        self.props.row = Some(rows[at].row.key.clone());
        self.props.popup = None;
        Outcome::Done
    }

    /// `props.category`: `category` (its word) of the section of record kind `kind` (else of the focused row's
    /// section; else the focused row's category) and `expanded` (else it toggles). Collapse states persist per (kind,
    /// category) in the settings file. A click on a category's header names its section: a selected symbol shows a
    /// device's, a function's and a representation's section, and the header clicked is not the focused row's.
    pub(crate) fn act_props_category(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(sheet) = self.props.sheet.clone() else {
            return Outcome::refused(key!("reason-no-sheet"));
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
                    return Outcome::refused(key!("reason-no-row"));
                }
            },
        };
        let section = match string_param(p, "kind") {
            Err(e) => return e,
            Ok(None) => section,
            Ok(Some(kind)) => match sheet.sections.iter().find(|s| s.kind == kind) {
                Some(s) => s,
                None => return failed(key!("error-bad-param"), &["kind"]),
            },
        };
        let group = match string_param(p, "category") {
            Err(e) => return e,
            Ok(Some(word)) => match crate::properties::RowGroup::from_word(word) {
                Some(g) => g,
                None => return failed(key!("error-bad-param"), &["category"]),
            },
            Ok(None) => crate::properties::group_of(section, row),
        };
        let key = crate::properties::group_key(&section.kind, group);
        let collapsed = self.properties_collapsed().contains(&key);
        let expand = p.get("expanded").and_then(Value::as_bool).unwrap_or(collapsed);
        if self.set_group_open(key, expand) {
            self.save_settings();
        }
        Outcome::Done
    }
}

#[cfg(test)]
pub(super) mod tests {
    use std::path::Path;

    use ew_api::{Origin, Submit};
    use serde_json::json;

    use super::*;
    use crate::properties::{EditRoute, PendingEdit};
    use crate::shell::Mode;

    /// A typed tag that changes only the letter or only the counter goes to that row; anything else to the
    /// Tag row.
    #[test]
    fn tag_edit_of_reads_the_letter_or_the_counter() {
        let letter = |v: &str| Some(("device/tag.letter", v.to_owned()));
        let counter = |v: &str| Some(("device/tag.counter", v.to_owned()));
        assert_eq!(tag_edit_of("-102F1", "F", "1", "-102K1"), letter("K"));
        assert_eq!(tag_edit_of("-102F1", "F", "1", " -102F2 "), counter("2"));
        assert_eq!(tag_edit_of("-X1031", "X", "1", "-X1032"), counter("2"));
        assert_eq!(tag_edit_of("-X1031", "X", "1", "-Y1031"), letter("Y"));
        assert_eq!(tag_edit_of("-102F1", "F", "1", "-102F1"), None, "the same tag");
        assert_eq!(tag_edit_of("-102F1", "F", "1", "-102K7"), None, "both changed");
        assert_eq!(tag_edit_of("-102F1", "F", "1", "-101F1"), None, "another page");
        assert_eq!(tag_edit_of("-102F1", "F", "1", "-102F1a"), None);
        assert_eq!(tag_edit_of("-102F1", "F", "", "-102F2"), None, "no counter");
    }

    pub(in crate::shell) fn copy_dir(from: &Path, to: &Path) {
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

    /// The grid never builds a command whose exposure is not `Edit`. A pending edit naming the `HumanOnly`
    /// `project.checks.set` (no row routes there, so only a forged edit can) is refused before any preview.
    #[test]
    fn a_human_only_command_is_refused_before_its_preview() {
        let mut shell = ShellModel::for_tests();
        let base = ew_testdir::Tmp::new("ew-d4");
        let dir = base.join("g.ew");
        let sample = Path::new(ew_testdir::SAMPLE);
        copy_dir(sample, &dir);
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
            Outcome::refused(key!("reason-human-only"))
        );
        let after = shell.session_calls();
        assert_eq!(after.preview_with, before.preview_with, "no preview");
        assert_eq!(after.commit, before.commit);
        assert_eq!(after.run_script, before.run_script);
        // A refusal at `/targets/<i>` names that target by its label, also among several targets.
        assert_eq!(
            shell.dispatch("select.set", &json!({"refs": ["tag:-101F1", "tag:-102F1"]})),
            Outcome::Done
        );
        let section = shell.props.sheet().unwrap().sections[0].clone();
        let err = CmdError::invalid_params(
            "fields.set",
            "/targets/1",
            ew_commands::text!("refusal-params-detail", "refused"),
        );
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
            refusal_text(
                &CmdError::invalid_params(
                    "fields.set",
                    "/value",
                    ew_commands::text!("refusal-params-detail", "bad"),
                ),
                &|_| String::new()
            ),
            "bad"
        );
        shell.shutdown();
        let _ = std::fs::remove_dir_all(&base);
    }

    /// The shell's undo and redo carry its origin. A `project.checks.set` step committed from the GUI is
    /// refused to the headless script (`Origin::CmdLog`) with the history reason key, the session unchanged; the
    /// window's GUI origin undoes and redoes it.
    #[test]
    fn a_human_only_step_is_undone_only_in_the_window() {
        let mut shell = ShellModel::for_tests();
        let base = ew_testdir::Tmp::new("ew-ft404");
        let dir = base.join("g.ew");
        let sample = Path::new(ew_testdir::SAMPLE);
        copy_dir(sample, &dir);
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
        let refused = Outcome::refused(key!("reason-human-only-history"));
        assert_eq!(shell.dispatch("edit.undo", &Value::Null), refused);
        assert_eq!(
            (cursor(&shell), rev(&shell)),
            (applied, at),
            "a refused undo changes nothing"
        );
        assert_eq!(
            strings::text(key!("reason-human-only-history")),
            "Only a person undoes or redoes this step, in the window: it holds a human-only command that decides what \
             the checks report",
            "the reason says why, with no plan reference"
        );
        // The window's shell: a person drives it (the origin is `Origin::Gui` only there).
        shell.mode = Mode::Gui;
        shell.person = true;
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
        let _ = std::fs::remove_dir_all(&base);
    }
}
