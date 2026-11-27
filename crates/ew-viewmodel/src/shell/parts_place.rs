// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): a screen of its own ([`Screen::Parts`](super::Screen::Parts)) with the
//! list, the open part's sheet and its pictures. `parts.open {part?, section?}` opens it on a part (or on the list),
//! from wherever a part is met; `parts.find` changes the list; `parts.new` makes a part (it asks What is it? first,
//! or a form names its start; like another part; for a symbol); `parts.field` and `parts.row` change the open sheet; `parts.save`
//! saves it where it lives (a new part into My parts, a part of My parts as its next version with the card of what
//! changes for the project, a part only in this project in place); `parts.discard` drops the changes; `parts.own`
//! makes a part of one's own from a part that is not; `parts.keep` puts a part only in this project into My parts;
//! `parts.remove` takes a part out of My parts or out of the project; `parts.back` returns to where the person came
//! from.

use ew_api::output::{NavKind, NavRole, Uuid};
use ew_api::part_fields::{Group, Lack};
use ew_api::parts::{Lives, PartFound, PartScope, start_named};
use ew_api::{OfferView, PartDraftError, PartOfferQuery, PartPick, PartSheet};
use serde_json::{Map, Value};

use super::part_dialog::{BusPart, PartChooser, card_fields, draft_refusal, matches_text, plug_description};
use crate::key;
use crate::properties::{CardOrigin, PartCard};
use crate::shell::{Dialog, FormDialog, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::strings;

/// Where a part lives, in words: Mine · in My parts, Starter part, the library's name, Only in this project, New.
pub fn lives_word(lives: &Lives) -> String {
    match lives {
        Lives::Mine => strings::text(key!("parts-lives-mine")).to_owned(),
        Lives::NotMine { starter: true, .. } => strings::text(key!("parts-lives-starter")).to_owned(),
        Lives::NotMine { library, .. } => library.clone(),
        Lives::OnlyHere => strings::text(key!("parts-lives-here")).to_owned(),
        Lives::New => strings::text(key!("parts-lives-new")).to_owned(),
    }
}

/// The most rows the list shows at once.
pub const LIST_ROWS: usize = 200;

/// Where the person came from, which the strip over the sheet names and Back returns to.
#[derive(Clone, PartialEq, Debug)]
pub enum Origin {
    /// The project's window.
    Project,
    /// The part dialog choosing a part for devices: Save reads Save and use for, Back returns to the dialog.
    Chooser(Box<PartChooser>),
    /// A form that needs a part that is not there (a cable's type, a bus cable, a plug): Save returns to it with the
    /// new part chosen, Back without it.
    Form(Box<FormDialog>),
    /// The device browser of a page, whose New part… made a part: Save returns to the page.
    Browser,
    /// A station's view, whose catalogue asked for a new card or head: Save returns to it with the new part in the
    /// catalogue (its head when the empty head was chosen).
    Station,
}

/// The open part of the place.
#[derive(Clone, PartialEq, Debug)]
pub struct OpenSheet {
    /// The part as a value, its texts and their messages.
    pub sheet: PartSheet,
    /// Where it lives.
    pub lives: Lives,
    /// What it lacks, as the texts give it now (Ready for, the dots of the contents list).
    pub lacks: Vec<Lack>,
    /// The makers the Maker field offers.
    pub manufacturers: Vec<String>,
    /// The refusal of the last Save, in words.
    pub error: Option<String>,
    /// New bus cable… or New plug… of a bus line's Cable form: what the part is made as.
    pub bus_part: Option<BusPart>,
    /// The group the place shows first (a quick fix's Edit part… at In the cabinet).
    pub group: Option<Group>,
    /// Whether a text was typed since it was opened or saved.
    pub changed: bool,
    /// The number of this opening among all the place's openings: the window shows a part opened anew from its top.
    pub opened: u64,
}

impl OpenSheet {
    /// Whether the person may change the part's fields: his own, or new.
    pub fn editable(&self) -> bool {
        matches!(self.lives, Lives::Mine | Lives::OnlyHere | Lives::New)
    }

    /// The heading: the part number, or New part.
    pub fn title(&self) -> String {
        let number = self.sheet.text("part_number").trim();
        if number.is_empty() || (self.sheet.new && number == "-") {
            strings::text(key!("parts-new-title")).to_owned()
        } else {
            number.to_owned()
        }
    }

    /// The words of the last Save's refusal, else the newest message of a field after the field's words.
    pub fn foot_message(&self) -> Option<String> {
        if let Some(error) = &self.error {
            return Some(error.clone());
        }
        let m = (self.sheet.messages.iter().rev()).find(|m| !m.field.starts_with("plc."))?;
        Some(strings::format(
            key!("part-form-message"),
            &[
                &super::part_dialog::field_label(&m.field),
                &super::part_dialog::message_text(strings::Key::run_time(m.key), &m.args),
            ],
        ))
    }
}

/// What New part… asks before a sheet opens (doc 07 §5.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Asking {
    /// What is it?: the cards of the starts.
    WhatIsIt,
    /// A part like it…: the part to start from, opened from the list.
    LikeWhich,
}

/// The place's state.
#[derive(Clone, PartialEq, Debug)]
pub struct PartsPlace {
    /// Where the person came from.
    pub origin: Origin,
    /// The list's search.
    pub query: String,
    /// The list's scope.
    pub scope: PartScope,
    /// The list's kind filter (a kind as the file spells it).
    pub kind: Option<String>,
    /// Whether the list shows only the parts that lack something.
    pub lacking: bool,
    /// The list's rows.
    pub found: Vec<PartFound>,
    /// The open part.
    pub sheet: Option<OpenSheet>,
    /// What New part… asks while no sheet is open.
    pub asking: Option<Asking>,
}

impl PartsPlace {
    /// The strip's sentence over the sheet: where the person came from with a purpose; `None` from the project's window.
    pub fn strip(&self) -> Option<String> {
        match &self.origin {
            Origin::Project | Origin::Station | Origin::Browser => None,
            Origin::Chooser(c) => Some(strings::format(key!("parts-strip-choosing"), &[&c.target_text])),
            Origin::Form(f) => Some(strings::format(
                key!("parts-strip-form"),
                &[strings::text(f.title_key)],
            )),
        }
    }

    /// The label of the way back: Back without choosing, Back to the form, Back.
    pub fn back_label(&self) -> &'static str {
        match &self.origin {
            Origin::Project | Origin::Station | Origin::Browser => strings::text(key!("parts-back")),
            Origin::Chooser(_) => strings::text(key!("parts-back-without-choosing")),
            Origin::Form(_) => strings::text(key!("parts-back-without-it")),
        }
    }

    /// The label of Save: Save and use for the chooser's target from a chooser, else Save.
    pub fn save_label(&self) -> String {
        match &self.origin {
            Origin::Chooser(c) => strings::format(key!("parts-save-use"), &[&c.target_text]),
            _ => strings::text(key!("parts-save")).to_owned(),
        }
    }
}

/// A part named by `parts.open`'s `part`: a copy the project holds, or a library's part.
enum Named {
    Held(Uuid),
    Library(Uuid, Uuid),
}

impl ShellModel {
    /// The place, when it is shown.
    pub fn parts_place(&self) -> Option<&PartsPlace> {
        self.parts.as_deref()
    }

    /// The part `parts.open`'s `part` names: `id:<copy>`, `lib:<library>/<item>`, or a part number the project holds;
    /// without it, the Libraries navigator's part row when it has the focus, else the property grid's device's first
    /// part use, else that navigator's cursor row.
    fn named_part(&self, p: &Map<String, Value>) -> Result<Option<Named>, Outcome> {
        let Some(project) = &self.project else {
            return Ok(None);
        };
        if let Some(text) = string_param(p, "part")? {
            let text = text.trim();
            if let Some(id) = text.strip_prefix("id:") {
                return Ok(id.parse().ok().map(Named::Held));
            }
            if let Some((library, item)) = text.strip_prefix("lib:").and_then(|r| r.split_once('/')) {
                return match (library.parse(), item.parse()) {
                    (Ok(l), Ok(i)) => Ok(Some(Named::Library(l, i))),
                    _ => Err(failed(key!("error-bad-param"), &["part"])),
                };
            }
            let held = project.session.part_offers(&PartOfferQuery {
                view: OfferView::Project,
                // Every kind, a cable gland too.
                kinds: ew_api::part_kinds(),
                include_inactive: true,
                lang: Some(strings::rule_language().to_owned()),
                ..PartOfferQuery::default()
            });
            let hits: Vec<Uuid> = held
                .iter()
                .filter(|o| matches_text(text, &o.part_number, &o.manufacturer, &o.description))
                .filter_map(|o| match o.pick {
                    PartPick::Held(c) => Some(c),
                    PartPick::Library { .. } => None,
                })
                .collect();
            return match hits.as_slice() {
                [one] => Ok(Some(Named::Held(*one))),
                [] => Err(failed(key!("error-part-unknown"), &[text])),
                _ => Err(failed(key!("error-accessory-part-ambiguous"), &[text])),
            };
        }
        let lib_row = || {
            self.nav_cursor(NavKind::Libraries).and_then(|k| {
                let nav = self.generic(NavKind::Libraries)?;
                let (_, n) = nav.node_of(&k)?;
                (n.role == NavRole::LibCopy)
                    .then(|| n.records.first().copied())
                    .flatten()
            })
        };
        if self.focused_navigator() == Some(NavKind::Libraries)
            && let Some(c) = lib_row()
        {
            return Ok(Some(Named::Held(c)));
        }
        let grid = self.props.sheet().and_then(|sheet| {
            let section = sheet.sections.iter().find(|s| s.kind == "device")?;
            match &section.rows.iter().find(|r| r.ty.is_part_uses())?.value {
                ew_api::output::RowValue::One { json, .. } => json
                    .get(0)?
                    .get("part")?
                    .get("copy")?
                    .as_str()?
                    .parse::<Uuid>()
                    .ok(),
                _ => None,
            }
        });
        Ok(grid.or_else(lib_row).map(Named::Held))
    }

    /// The copy a part action names (`id:<copy>`, a part number the project holds, else the Libraries navigator's or
    /// the selected device's part); `None` for none or a library's part the project does not hold.
    pub(crate) fn named_copy(&self, p: &Map<String, Value>) -> Result<Option<Uuid>, Outcome> {
        Ok(match self.named_part(p)? {
            Some(Named::Held(c)) => Some(c),
            _ => None,
        })
    }

    /// The place shown over the project, from where the person is now: the part dialog (the strip names its target)
    /// or a form that needs a part; it keeps its list when it is shown already.
    pub(crate) fn enter_place(&mut self) {
        if self.parts.is_some() {
            return;
        }
        let origin = match self.dialog.take() {
            Some(Dialog::PartChoose(c)) => Origin::Chooser(c),
            Some(Dialog::Form(f))
                if super::cables::chooses_cable_type(&f) || f.action == super::cables::BUS_CABLE =>
            {
                Origin::Form(Box::new(f))
            }
            other => {
                self.dialog = other;
                if self.tools.picker.is_some() {
                    Origin::Browser
                } else if self.station_tab().is_some() {
                    Origin::Station
                } else {
                    Origin::Project
                }
            }
        };
        self.tools.picker = None;
        self.parts = Some(Box::new(PartsPlace {
            origin,
            query: String::new(),
            scope: PartScope::All,
            kind: None,
            lacking: false,
            found: Vec::new(),
            sheet: None,
            asking: None,
        }));
        self.parts_refresh();
    }

    /// The list again, from the place's query.
    pub(crate) fn parts_refresh(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let Some(place) = self.parts.as_deref_mut() else {
            return;
        };
        place.found = project.session.parts_find(
            &place.query,
            place.scope,
            place.kind.as_deref(),
            place.lacking,
            Some(strings::rule_language().to_owned()),
            LIST_ROWS,
        );
        if let Some(open) = &mut place.sheet {
            open.lacks = project.session.sheet_lacks(&open.sheet);
        }
    }

    /// Opens `sheet` in the place, where it lives.
    pub(crate) fn open_sheet(&mut self, sheet: PartSheet, bus_part: Option<BusPart>) {
        let Some(project) = &self.project else {
            return;
        };
        let lives = project.session.sheet_lives(&sheet);
        let lacks = project.session.sheet_lacks(&sheet);
        let manufacturers = project.session.part_manufacturers();
        self.parts_opened = self.parts_opened.wrapping_add(1);
        let opened = self.parts_opened;
        if let Some(place) = self.parts.as_deref_mut() {
            place.asking = None;
            place.sheet = Some(OpenSheet {
                opened,
                sheet,
                lives,
                lacks,
                manufacturers,
                error: None,
                bus_part,
                group: None,
                changed: false,
            });
        }
    }

    /// `parts.open {part?, section?}` (Parts, Edit part…, Open part): shows the place, on the part named (see
    /// [`ShellModel::named_part`]) when there is one, else on its list; with `section`, at that group of the sheet.
    pub(crate) fn act_parts_open(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let named = match self.named_part(p) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let group = match string_param(p, "section") {
            Ok(Some(word)) => match Group::named(word) {
                Some(g) => Some(g),
                None => return failed(key!("error-bad-param"), &["section"]),
            },
            Ok(None) => None,
            Err(e) => return e,
        };
        let sheet = match (&self.project, named) {
            (Some(project), Some(Named::Held(c))) => match project.session.part_draft(c) {
                Ok(s) => Some(s),
                Err(_) => return Outcome::refused(key!("reason-part-edit-target")),
            },
            (Some(project), Some(Named::Library(l, i))) => match project.session.library_part_draft(l, i) {
                Ok(s) => Some(s),
                Err(_) => return Outcome::refused(key!("reason-part-edit-target")),
            },
            _ => None,
        };
        self.enter_place();
        let like = self.parts.as_deref().and_then(|p| p.asking) == Some(Asking::LikeWhich);
        if let Some(sheet) = sheet {
            if like && let Some(project) = &self.project {
                return match project.session.new_part_draft_copying(&sheet) {
                    Ok(mut new) => {
                        let _ = project.session.set_draft_field(&mut new, "part_number", "");
                        new.messages.clear();
                        self.open_sheet(new, None);
                        Outcome::Done
                    }
                    Err(e) => draft_refusal(e),
                };
            }
            self.open_sheet(sheet, None);
            if let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) {
                open.group = group;
            }
        }
        Outcome::Done
    }

    /// `parts.find {text?, scope?, kind?, lacking?}`: the list's search, scope (`all`, `project`, `mine`), kind filter
    /// (`all` for none) and Lacks something.
    pub(crate) fn act_parts_find(&mut self, p: &Map<String, Value>) -> Outcome {
        let text = or_return!(string_param(p, "text")).map(str::to_owned);
        let scope = match or_return!(string_param(p, "scope")) {
            None => None,
            Some(w) => match PartScope::ALL.into_iter().find(|s| s.word() == w) {
                Some(s) => Some(s),
                None => return failed(key!("error-bad-param"), &["scope"]),
            },
        };
        let kind = or_return!(string_param(p, "kind")).map(str::to_owned);
        let Some(place) = self.parts.as_deref_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        if let Some(t) = text {
            place.query = t;
        }
        if let Some(s) = scope {
            place.scope = s;
        }
        if let Some(k) = kind {
            place.kind = (k != "all").then_some(k);
        }
        if let Some(v) = p.get("lacking") {
            place.lacking = v.as_bool().unwrap_or(false);
        }
        self.parts_refresh();
        Outcome::Done
    }

    /// `parts.new {start?, like?, symbol?, bus?, field?}` (New part…, a card of What is it?, Like this, A part like
    /// it…, New cable type…, New bus cable…, New plug…, a station's New part…): a new part's sheet in the place.
    /// Without a start it asks What is it? first; `start` begins the sheet as that start of [`ew_api::parts::STARTS`]
    /// presets it; `like` starts it as the open part but for its number, with no part open it asks for the part to
    /// start from; `symbol` draws it with a saved symbol; `bus` makes the cable or the plug (start `plug`) for that bus,
    /// and Save returns to the bus line's form's field `field`.
    pub(crate) fn act_parts_new(&mut self, p: &Map<String, Value>) -> Outcome {
        let symbol = or_return!(string_param(p, "symbol")).map(str::to_owned);
        let start = match or_return!(string_param(p, "start")) {
            None => None,
            Some(name) => match start_named(name) {
                Some(s) => Some(s),
                None => return failed(key!("error-bad-param"), &["start"]),
            },
        };
        let like = flag(p, "like");
        let project = or_return!(crate::shell::gated(&self.project));
        let open = self.parts.as_deref().and_then(|p| p.sheet.as_ref());
        let made = match (&symbol, start, like) {
            (Some(address), _, _) => project.session.new_part_draft_for_symbol(address),
            (None, Some(start), _) => project.session.start_sheet(start),
            (None, None, true) if open.is_some() => {
                let held = &open.expect("checked").sheet;
                project.session.new_part_draft_copying(held).map(|mut d| {
                    let _ = project.session.set_draft_field(&mut d, "part_number", "");
                    d.messages.clear();
                    d
                })
            }
            (None, None, _) => {
                // What is it?, or the part to start from: the place asks before a sheet opens.
                self.enter_place();
                if let Some(place) = self.parts.as_deref_mut() {
                    place.sheet = None;
                    place.asking = Some(if like { Asking::LikeWhich } else { Asking::WhatIsIt });
                }
                return Outcome::Done;
            }
        };
        let mut sheet = match made {
            Ok(d) => d,
            Err(e) => return draft_refusal(e),
        };
        let set = |sheet: &mut PartSheet, f: &str, v: &str| {
            let _ = project.session.set_draft_field(sheet, f, v);
        };
        let bus_part = match or_return!(string_param(p, "bus")) {
            None => None,
            Some(bus) => {
                let plug = start.is_some_and(|s| s.name == "plug");
                let field = match or_return!(string_param(p, "field")) {
                    Some("plug_a") => "plug_a",
                    Some("plug_b") => "plug_b",
                    _ if plug => "plug_a",
                    _ => "part",
                };
                if plug {
                    for (f, v) in [("bus.0.bus", bus), ("bus.0.termination", "")] {
                        set(&mut sheet, f, v);
                    }
                } else {
                    set(&mut sheet, "cable.bus", bus);
                }
                let made = BusPart {
                    plug,
                    field,
                    angled: false,
                    socket: false,
                };
                if plug {
                    let words = plug_description(&sheet, made);
                    set(&mut sheet, "description", &words);
                }
                Some(made)
            }
        };
        // From the symbol editor, its view is left with its draft kept (as Close editor does).
        if symbol.is_some()
            && let Some(editor) = &mut self.symbol_editor
        {
            editor.shown = false;
        }
        self.enter_place();
        self.open_sheet(sheet, bus_part);
        Outcome::Done
    }

    /// The open sheet, mutably.
    fn open_mut(&mut self) -> Option<&mut OpenSheet> {
        self.parts.as_deref_mut()?.sheet.as_mut()
    }

    /// After a text of the open sheet changed: changed, and what it lacks again.
    fn sheet_changed(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        if let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) {
            open.changed = true;
            open.error = None;
            open.lacks = project.session.sheet_lacks(&open.sheet);
        }
    }

    /// `parts.field {field, value}`: sets one field of the open sheet; a message shows under the field. A part that is
    /// not the person's own refuses in words: he makes his own from it.
    pub(crate) fn act_parts_field(&mut self, p: &Map<String, Value>) -> Outcome {
        let (field, value) = match (string_param(p, "field"), string_param(p, "value")) {
            (Ok(Some(f)), Ok(Some(v))) => (f.to_owned(), v.to_owned()),
            (Ok(None), _) => return failed(key!("error-missing-param"), &["field"]),
            (_, Ok(None)) => return failed(key!("error-missing-param"), &["value"]),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if !open.editable() {
            return Outcome::refused(key!("reason-part-not-yours"));
        }
        // A new plug's cable outlet and programming socket are words of its description, which follows them, its
        // bus and its terminating resistor.
        if let Some(mut plug) = open.bus_part.filter(|b| b.plug) {
            match field.as_str() {
                "plug.angled" => plug.angled = value == "true",
                "plug.socket" => plug.socket = value == "true",
                _ => {}
            }
            open.bus_part = Some(plug);
            if !field.starts_with("plug.")
                && let Err(PartDraftError::Refused(_)) =
                    project.session.set_draft_field(&mut open.sheet, &field, &value)
            {
                return failed(key!("error-part-field"), &[&field]);
            }
            if field.starts_with("plug.") || field.starts_with("bus.") {
                let words = plug_description(&open.sheet, plug);
                let _ = project
                    .session
                    .set_draft_field(&mut open.sheet, "description", &words);
            }
            self.sheet_changed();
            return Outcome::Done;
        }
        // A module starts without a bus interface: the port a station's start put in goes when the role is made
        // Module, while it is as the start made it.
        if field == "plc.role" && value == "module" && open.sheet.new && open.sheet.port_as_started() {
            open.sheet.remove_bus_row(0);
        }
        // The front's imported outline is removed by an empty text.
        if field == "mech.front_2d" {
            if !value.is_empty() || !open.sheet.remove_outline() {
                return failed(key!("error-part-field"), &[&field]);
            }
            self.sheet_changed();
            return Outcome::Done;
        }
        let out = match project.session.set_draft_field(&mut open.sheet, &field, &value) {
            Err(PartDraftError::Refused(_)) => failed(key!("error-part-field"), &[&field]),
            Ok(()) | Err(PartDraftError::Field(_)) => Outcome::Done,
        };
        if out == Outcome::Done {
            self.sheet_changed();
        }
        out
    }

    /// `parts.row {table, remove?}`: adds a row to the open sheet's table `table` (`accessories`, `variants`,
    /// `mech.holes`, `sections.enclosure.doors`, …, `bus` the network ports), or removes row `remove` (1-based).
    pub(crate) fn act_parts_row(&mut self, p: &Map<String, Value>) -> Outcome {
        let table = match string_param(p, "table") {
            Ok(Some(t)) => t.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["table"]),
            Err(e) => return e,
        };
        let remove = match p.get("remove") {
            None => None,
            Some(v) => match v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .filter(|i| *i >= 1)
            {
                Some(i) => Some(i - 1),
                None => return failed(key!("error-bad-param"), &["remove"]),
            },
        };
        let Some(open) = self.open_mut() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if !open.editable() {
            return Outcome::refused(key!("reason-part-not-yours"));
        }
        let done = if table == "bus" {
            match remove {
                None => {
                    open.sheet.add_bus_row();
                    true
                }
                Some(i) => open.sheet.remove_bus_row(i),
            }
        } else {
            let editable = open
                .sheet
                .table(&table)
                .is_some_and(|t| open.sheet.section_editable(t.group));
            if !editable {
                return failed(key!("error-part-field"), &[&table]);
            }
            match remove {
                None => open.sheet.add_table_row(&table),
                Some(i) => open.sheet.remove_table_row(&table, i),
            }
        };
        if !done {
            return failed(
                key!("error-bad-param"),
                &[if remove.is_some() { "remove" } else { "table" }],
            );
        }
        self.sheet_changed();
        Outcome::Done
    }

    /// `parts.outline {path?}` (Import outline… of In the cabinet, doc 07 §5.4): the front outline of the DXF drawing at
    /// `path` into the open sheet, drawn instead of the front's shape; without `path` the file dialog asks. Refused in
    /// words where the part is not the person's to change, the file cannot be read or holds no line; Save writes it.
    pub(crate) fn act_parts_outline(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("parts.outline", Prompt::OpenOutline, "path", p),
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        match project.session.draft_outline_import(&mut open.sheet, &path) {
            Ok(_) => {
                self.sheet_changed();
                Outcome::Done
            }
            Err(message) => {
                open.error = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    /// `parts.discard`: the open part as it was opened (a new part's sheet closes).
    pub(crate) fn act_parts_discard(&mut self) -> Outcome {
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        if open.sheet.new {
            if let Some(place) = self.parts.as_deref_mut() {
                place.sheet = None;
            }
            return Outcome::Done;
        }
        let copy = open.sheet.copy;
        let source = open.sheet.source;
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let again = match source {
            Some((l, i)) => project.session.library_part_draft(l, i),
            None => project.session.part_draft(copy),
        };
        match again {
            Ok(sheet) => {
                self.open_sheet(sheet, None);
                Outcome::Done
            }
            Err(_) => Outcome::refused(key!("reason-part-edit-target")),
        }
    }

    /// `parts.own` (Make my own from it): a new part's sheet with everything the open part has but its number and
    /// maker, which are his to give; Save makes it a part of My parts.
    pub(crate) fn act_parts_own(&mut self) -> Outcome {
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let mut sheet = match project.session.new_part_draft_copying(&open.sheet) {
            Ok(s) => s,
            Err(e) => return draft_refusal(e),
        };
        for field in ["part_number", "manufacturer"] {
            let _ = project.session.set_draft_field(&mut sheet, field, "");
        }
        sheet.messages.clear();
        self.open_sheet(sheet, None);
        Outcome::Done
    }

    /// `parts.keep` (Keep in my parts): the open part, only in this project, written into My parts, and the project's
    /// devices given that part of his (one undo step).
    pub(crate) fn act_parts_keep(&mut self) -> Outcome {
        let Some(copy) = self
            .parts
            .as_deref()
            .and_then(|p| p.sheet.as_ref())
            .filter(|o| o.lives == Lives::OnlyHere)
            .map(|o| o.sheet.copy)
        else {
            return Outcome::refused(key!("reason-part-keep"));
        };
        let project = or_return!(crate::shell::gated_mut(&mut self.project));
        let cmd = match project.session.keep_in_my_parts(copy) {
            Ok(c) => c,
            Err(e) => return draft_refusal(e),
        };
        let out = self.run_commit(&cmd);
        if out == Outcome::Done {
            self.libraries_changed();
            let kept = cmd.params["part"]
                .as_str()
                .or_else(|| cmd.params["copies"][0]["id"].as_str())
                .and_then(|s| s.parse::<Uuid>().ok());
            if let (Some(project), Some(kept)) = (&self.project, kept)
                && let Ok(sheet) = project.session.part_draft(kept)
            {
                self.open_sheet(sheet, None);
            }
            self.parts_refresh();
        }
        out
    }

    /// `parts.back`: returns to where the person came from: the part dialog with the open part under its cursor, the
    /// form that asked for a part, else the project's window. Refused in words while the open part has unsaved
    /// changes: Save or Discard first.
    pub(crate) fn act_parts_back(&mut self) -> Outcome {
        let Some(place) = self.parts.as_deref() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        if place.sheet.as_ref().is_some_and(|o| o.changed) {
            return Outcome::refused(key!("reason-parts-unsaved"));
        }
        let place = *self.parts.take().expect("checked");
        match place.origin {
            Origin::Project | Origin::Station | Origin::Browser => {}
            Origin::Chooser(c) => {
                self.dialog = Some(Dialog::PartChoose(c));
                self.dialog_gen += 1;
                self.part_refresh();
            }
            Origin::Form(f) => {
                self.dialog = Some(Dialog::Form(*f));
                self.dialog_gen += 1;
            }
        }
        Outcome::Done
    }

    /// `parts.remove {confirm?}` (Remove…): takes the open part out of My parts (the project keeps its copy, the toast
    /// names its devices), or out of the project when it is only there and no device uses it. Undo does not bring a part
    /// of My parts back, so without `confirm` the window asks first and a script is refused in words. A part that is
    /// not his is refused in words.
    pub(crate) fn act_parts_remove(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let (copy, number, lives) = (
            open.sheet.copy,
            open.sheet.text("part_number").to_owned(),
            open.lives.clone(),
        );
        match lives {
            Lives::OnlyHere => {
                let cmd = ew_api::AnyCommand {
                    name: "part.delete".into(),
                    params: serde_json::json!({ "copy": copy.to_string() }),
                };
                let out = self.run_commit(&cmd);
                if out == Outcome::Done {
                    if let Some(place) = self.parts.as_deref_mut() {
                        place.sheet = None;
                    }
                    self.parts_refresh();
                    self.notify(strings::format(key!("part-deleted"), &[&number]), false);
                }
                out
            }
            Lives::Mine => {
                let Some((library, item)) = self.mine_item(copy) else {
                    return Outcome::refused(key!("reason-part-not-own"));
                };
                if !flag(p, "confirm") {
                    return match self.mode {
                        crate::shell::Mode::Headless => failed(key!("error-part-remove-confirm"), &[]),
                        crate::shell::Mode::Gui => self.open_dialog(Dialog::ConfirmRemovePart {
                            part: format!("id:{copy}"),
                            number,
                        }),
                    };
                }
                let project = or_return!(crate::shell::gated_mut(&mut self.project));
                let removed = match project.session.remove_own_part(library, item) {
                    Ok(r) => r,
                    Err(e) => return draft_refusal(e),
                };
                if matches!(&self.dialog, Some(Dialog::ConfirmRemovePart { .. })) {
                    self.dialog = None;
                }
                if let Some(place) = self.parts.as_deref_mut() {
                    place.sheet = None;
                }
                self.libraries_changed();
                self.parts_refresh();
                let text = if removed.devices.is_empty() {
                    strings::format(key!("part-removed-own"), &[&removed.number])
                } else {
                    strings::format(
                        key!("part-removed-own-kept"),
                        &[&removed.number, &removed.devices.join(", ")],
                    )
                };
                self.notify(text, false);
                Outcome::Done
            }
            Lives::NotMine { .. } | Lives::New => Outcome::refused(key!("reason-part-not-own")),
        }
    }

    /// The library and item of the open part of My parts `copy` (held, or a library part opened from the list).
    fn mine_item(&self, copy: Uuid) -> Option<(Uuid, Uuid)> {
        let project = self.project.as_ref()?;
        let open = self.parts.as_deref()?.sheet.as_ref()?;
        let held = {
            let q = project.session.query();
            copy.to_string().parse().ok().and_then(|id| {
                q.parts()
                    .get(id)
                    .map(|c| (c.library_id.as_uuid(), c.item_id.as_uuid()))
            })
        };
        let (library, item) = held.or(open.sheet.source)?;
        project.session.own_part_number(library, item)?;
        Some((library, item))
    }

    /// `parts.save {another?}` (Save, Save and use for): saves the open part where it lives. A new part goes into My
    /// parts and the project at once (from the part dialog it is then given to the devices it was chosen for; from a
    /// form, the form is back with it chosen; with `another`, a new cable type's next size opens); a part of My parts
    /// writes its next version there and opens the card of what changes for the project; a part only in this project
    /// opens the card of its correction. A field that does not parse refuses at its field; a part that is not his is
    /// refused in words.
    pub(crate) fn act_parts_save(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        match open.lives {
            Lives::New => self.parts_save_new(flag(p, "another")),
            Lives::Mine => self.parts_save_mine(),
            Lives::OnlyHere => self.parts_save_here(),
            Lives::NotMine { .. } => Outcome::refused(key!("reason-part-not-yours")),
        }
    }

    /// Puts a refusal of Save under the open sheet, at its field when it names one, and returns it.
    fn save_refused(&mut self, e: PartDraftError) -> Outcome {
        let Some(open) = self.open_mut() else {
            return draft_refusal(e);
        };
        if let PartDraftError::Field(m) = &e {
            open.sheet.messages.retain(|x| x.field != m.field);
            open.sheet.messages.push(m.clone());
        }
        let out = draft_refusal(e);
        open.error = Some(match &out {
            Outcome::Refused { reason } => strings::render(reason),
            Outcome::Failed { message } => message.clone(),
            Outcome::Done => String::new(),
        });
        out
    }

    /// Save of a part of My parts: its next version goes into My parts, and the card of the `library.update` that
    /// brings the project's copy to it opens beside the sheet; Apply commits it.
    fn parts_save_mine(&mut self) -> Outcome {
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::Done;
        };
        let title = open.title();
        match project.session.own_part_update(&open.sheet) {
            Ok(None) => {
                // A part of My parts the project does not hold: its next version is in My parts, nothing else changes.
                if let Some(open) = self.open_mut() {
                    open.changed = false;
                }
                self.notify(strings::format(key!("parts-saved-mine"), &[&title]), false);
                self.libraries_changed();
                self.parts_refresh();
                Outcome::Done
            }
            Ok(Some(plan)) => {
                let fields = plan
                    .copies
                    .first()
                    .map(|c| card_fields(&c.fields))
                    .unwrap_or_default();
                let devices = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
                let card = PartCard {
                    title,
                    fields,
                    devices,
                    removed: plan.bom_removed,
                    added: plan.bom_added,
                    changed: plan.bom_changed,
                    boxes: self.box_words(&plan.boxes),
                    lost: plan.lost,
                    fork: false,
                };
                if let Some(open) = self.open_mut() {
                    open.changed = false;
                }
                self.preview_host_card_as(
                    &plan.command.name,
                    plan.command.params,
                    key!("reason-library-update"),
                    CardOrigin::Part(Box::new(card)),
                )
            }
            Err(e) => self.save_refused(e),
        }
    }

    /// Save of a part only in this project: the card of its correction (`part.edit_copy`, in place) opens beside the
    /// sheet; Apply commits it.
    fn parts_save_here(&mut self) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::Done;
        };
        let title = open.title();
        let plan = match project.session.part_edit_plan(&open.sheet) {
            Ok(plan) => plan,
            Err(e) => return self.save_refused(e),
        };
        let devices: Vec<String> = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        let card = PartCard {
            title,
            fields: card_fields(&plan.fields),
            devices,
            removed: plan.bom_removed.clone(),
            added: plan.bom_added.clone(),
            changed: plan.bom_changed.clone(),
            lost: plan.lost.clone(),
            boxes: self.box_words(&plan.boxes),
            fork: plan.fork.is_some(),
        };
        if let Some(open) = self.open_mut() {
            open.changed = false;
        }
        self.preview_host_card_as(
            &plan.command.name,
            plan.command.params,
            key!("reason-part-edit"),
            CardOrigin::Part(Box::new(card)),
        )
    }

    /// Save of a new part: into My parts and the project (`Session::new_part_command`), in one undo step. From the part
    /// dialog the part is given to its devices and the place gives way to the page; from a form, the form is back with
    /// the part chosen; with `another` a new part like this one but for its number and its cross-section opens (the
    /// next size of a series); else the place shows the saved part.
    fn parts_save_new(&mut self, another: bool) -> Outcome {
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::Done;
        };
        let made = match project.session.new_part_command(&open.sheet) {
            Ok(made) => made,
            Err(e) => return self.save_refused(e),
        };
        let out = self.run_commit(&made.command);
        if out != Outcome::Done {
            if let Some(open) = self.open_mut() {
                open.error = Some(match &out {
                    Outcome::Refused { reason } => strings::render(reason),
                    Outcome::Failed { message } => message.clone(),
                    Outcome::Done => String::new(),
                });
            }
            return out;
        }
        let open = self
            .parts
            .as_deref_mut()
            .and_then(|p| p.sheet.take())
            .expect("checked");
        let number = open.sheet.text("part_number").trim().to_owned();
        let toast = match &made.library {
            Some(name) => strings::format(key!("toast-part-new-library"), &[&number, name]),
            None => strings::format(key!("toast-part-new"), &[&number]),
        };
        self.notify(toast, true);
        self.libraries_changed();
        if another {
            return self.parts_new_like(open);
        }
        let origin = self
            .parts
            .as_deref()
            .map(|p| p.origin.clone())
            .unwrap_or(Origin::Project);
        match origin {
            Origin::Form(form) => {
                self.parts = None;
                match open.bus_part {
                    Some(b) => self.bus_part_made(*form, &number, b.field),
                    None => self.cable_type_made(*form, &number),
                }
                Outcome::Done
            }
            Origin::Chooser(mut chooser) => {
                // Save and use for: the part goes to the devices it was chosen for, and the place gives way.
                self.parts = None;
                chooser.query.text = number.clone();
                chooser.cursor = None;
                self.dialog = Some(Dialog::PartChoose(chooser));
                self.dialog_gen += 1;
                self.part_refresh();
                let pick = Map::from_iter([("part".to_owned(), Value::String(number))]);
                self.act_part_pick(&pick)
            }
            Origin::Browser => {
                // Made from a page's device browser: back to the page, where Device from part offers it.
                self.parts = None;
                Outcome::Done
            }
            Origin::Station => {
                // Made from the view of a station not made yet with its empty head chosen, a part that can head a
                // station is its head; else the part stands in the view's catalogue, offered.
                self.parts = None;
                let from_head = self
                    .station_tab()
                    .is_some_and(|t| t.cursor == Some(crate::area::StationCursor::Head));
                if from_head && self.station_draft().is_some_and(|d| d.head.is_none()) {
                    let head = Map::from_iter([("head".to_owned(), Value::String(number))]);
                    let _ = self.act_plc_rack_set(&head);
                }
                Outcome::Done
            }
            Origin::Project => {
                let copy = self.project.as_ref().and_then(|project| {
                    let q = project.session.query();
                    q.parts()
                        .iter()
                        .find(|(_, c)| c.item.part_number.as_str() == number)
                        .map(|(id, _)| id.as_uuid())
                });
                if let (Some(project), Some(copy)) = (&self.project, copy)
                    && let Ok(sheet) = project.session.part_draft(copy)
                {
                    self.open_sheet(sheet, None);
                }
                self.parts_refresh();
                Outcome::Done
            }
        }
    }

    /// A new part like the saved `saved` but for its number and its cross-section (Save and add another size): its own
    /// item ID, the form's way back kept.
    fn parts_new_like(&mut self, saved: OpenSheet) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let mut sheet = saved.sheet.clone();
        sheet.copy = match project.session.new_part_draft() {
            Ok(d) => d.copy,
            Err(e) => return draft_refusal(e),
        };
        for field in ["part_number", "cable.csa"] {
            let _ = project.session.set_draft_field(&mut sheet, field, "");
        }
        sheet.messages.clear();
        self.open_sheet(sheet, saved.bus_part);
        Outcome::Done
    }
}

impl ShellModel {
    /// The schematic picture of the open sheet's part as its texts give it now (doc 07 §5.6): its pieces drawn by
    /// their functions' symbols.
    pub fn part_schematic(
        &self,
        sheet: &PartSheet,
    ) -> Option<(ew_dl::DisplayList, (ew_units::Nm, ew_units::Nm))> {
        let session = &self.project.as_ref()?.session;
        session.part_schematic(&session.sheet_part(sheet))
    }
}
