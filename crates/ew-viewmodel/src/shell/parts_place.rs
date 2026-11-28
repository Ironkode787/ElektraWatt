// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parts, the one place for parts (doc 07 §5): a screen of its own ([`Screen::Parts`](super::Screen::Parts)) with the
//! list, the open part's sheet and its pictures. `parts.open {part?, field?}` opens it on a part (or on the list),
//! from wherever a part is met, at the field an outcome's Edit part… names; `parts.find` changes the list; `parts.new` makes a part (it asks What is it? first,
//! or a form names its start; like another part; for a symbol); `parts.field` and `parts.row` change the open sheet; `parts.save`
//! saves it where it lives (a new part into My parts, a part of My parts as its next version with the card of what
//! changes for the project, a part only in this project in place); `parts.discard` drops the changes; `parts.own`
//! makes a part of one's own from a part that is not; `parts.keep` puts a part only in this project into My parts;
//! `parts.remove` takes a part out of My parts or out of the project; `parts.back` returns to where the person came
//! from. A symbol is drawn in the same place, on its Drawing tab (`shell::symbol_editor`'s `place`); the list's Symbols
//! shelf lists the symbols to open.

use ew_api::output::{NavKind, NavRole, Uuid};
use ew_api::part_fields::{Group, Lack, PartLack};
use ew_api::parts::{Lives, PartFound, PartScope, start_named};
use ew_api::{OfferView, PartDraftError, PartOfferQuery, PartPick, PartSheet};
use serde_json::{Map, Value};

use super::part_dialog::{BusPart, PartChooser, card_fields, draft_refusal, matches_text, plug_description};
use crate::key;
use crate::properties::{CardOrigin, PartCard};
use crate::shell::{Dialog, FormDialog, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::strings;

/// The sentence that part names a part no one holds, by the fields' words (`missing` as
/// `ew_api::Session::names_missing` gives them), with the way; `None` for none.
pub fn missing_words(missing: &[&str]) -> Option<String> {
    if missing.is_empty() {
        return None;
    }
    let words: Vec<String> = (missing.iter())
        .filter_map(|p| ew_api::part_fields::field(p))
        .map(|f| strings::render(&ew_api::part_fields::word(f.word)))
        .collect();
    Some(strings::format(key!("parts-names-missing"), &[&words.join(", ")]))
}

/// Where a part lives in words ([`lives_word`]), and that My parts has it too where the project holds it only on a
/// copy of its own (a Keep taken back).
pub fn lives_words(lives: &Lives, mine_too: bool) -> String {
    let word = lives_word(lives);
    if mine_too {
        strings::format(key!("parts-lives-mine-too"), &[&word])
    } else {
        word
    }
}

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

/// The one line under a group's heading on the sheet of Parts (doc 16a §6.27), where the group has one.
pub fn group_line(group: Group) -> Option<&'static str> {
    let key = match group {
        Group::Schematic => key!("part-line-schematic"),
        Group::Cabinet => key!("part-line-cabinet"),
        Group::Connections => key!("part-line-connections"),
        Group::Ports => key!("part-line-ports"),
        Group::Plc => key!("part-line-plc"),
        Group::Technical => key!("part-line-technical"),
        Group::GoesWith => key!("part-line-goes-with"),
        _ => return None,
    };
    Some(strings::text(key))
}

/// The hint of a group's heading on the sheet of Parts, where the group has one.
pub fn group_hint(group: Group) -> Option<&'static str> {
    let key = match group {
        Group::Ports => key!("part-hint-ports"),
        Group::Plc => key!("part-hint-plc"),
        Group::Connections => key!("part-hint-connections"),
        Group::Cabinet => key!("part-hint-cabinet"),
        Group::Gland => key!("part-hint-gland"),
        _ => return None,
    };
    Some(strings::text(key))
}

/// The line over a table of the sheet of Parts, by the table's member (`accessories`), where it has one.
pub fn table_hint(path: &str) -> Option<&'static str> {
    let key = match path {
        "accessories" => key!("part-hint-accessories"),
        "variants" => key!("part-hint-variants"),
        _ => return None,
    };
    Some(strings::text(key))
}

/// The button that adds a row to a table of the sheet of Parts, by the table's member; `None` for a table whose rows
/// are not added there (the pins' table, whose rows the pins' form makes).
pub fn table_add_word(path: &str) -> Option<&'static str> {
    let key = match path {
        "accessories" => key!("part-table-add-accessories"),
        "variants" => key!("part-table-add-variants"),
        "mech.holes" => key!("part-table-add-mech-holes"),
        "sections.enclosure.doors" => key!("part-table-add-sections-enclosure-doors"),
        "configured.components" => key!("part-table-add-configured-components"),
        "sections.fluid.ports" => key!("part-table-add-sections-fluid-ports"),
        _ => return None,
    };
    Some(strings::text(key))
}

/// The symbols each piece of `sheet` can be drawn with, in the window's language ([`ew_api::Session::piece_symbols`]).
fn piece_symbols_of(session: &ew_api::Session, sheet: &PartSheet) -> Vec<Vec<(String, String)>> {
    let lang = strings::rule_language().split('-').next().unwrap_or("en");
    sheet
        .pieces
        .iter()
        .map(|p| session.piece_symbols(&p.what, lang))
        .collect()
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
    /// A finding's Edit part… in Messages, on the page in words where the finding is shown: Back returns to Messages.
    Finding {
        /// The page ("904 Door"), where the finding is shown on one.
        page: Option<String>,
    },
}

/// The open part of the place.
#[derive(Clone, PartialEq, Debug)]
pub struct OpenSheet {
    /// The part as a value, its texts and their messages.
    pub sheet: PartSheet,
    /// Where it lives.
    pub lives: Lives,
    /// Whether My parts has it too, while the project holds it only on a copy of its own: Keep in my parts brings the
    /// project onto My parts' copy.
    pub mine_too: bool,
    /// The fields that name a part neither the project nor a library he has holds, in words: said over Ready for with
    /// the way (keep that part from the project that holds it, or name another).
    pub missing: Option<String>,
    /// What it lacks, as the texts give it now (Ready for, the dots of the contents list).
    pub lacks: Vec<Lack>,
    /// Ready for: every feature that reads the part, one entry each, as the texts give it now.
    pub ready: Vec<ew_api::part_fields::Ready>,
    /// The project's devices that use the part, each with its tag (the heading's chip "-102Q1 uses it").
    pub used_by: Vec<(Uuid, String)>,
    /// The makers the Maker field offers.
    pub manufacturers: Vec<String>,
    /// The symbols each piece can be drawn with, by its address and its name, the starter's first
    /// (`Session::piece_symbols`): the Symbol cell's choices.
    pub piece_symbols: Vec<Vec<(String, String)>>,
    /// The refusal of the last Save, in words.
    pub error: Option<String>,
    /// New bus cable… or New plug… of a bus line's Cable form: what the part is made as.
    pub bus_part: Option<BusPart>,
    /// The group the place shows first: the one the field it was opened at stands in.
    pub group: Option<Group>,
    /// The field an outcome's Edit part… opened it at (doc 07 §5.7): ringed, with its sentence under it, while the
    /// part lacks it.
    pub field: Option<&'static str>,
    /// Whether a text was typed since it was opened or saved.
    pub changed: bool,
    /// The number of this opening among all the place's openings: the window shows a part opened anew from its top.
    pub opened: u64,
    /// Whether the project is open read-only: every part is shown, none changed.
    pub read_only: bool,
}

impl OpenSheet {
    /// Whether the person may change the part's fields: his own, or new, in a project he can write.
    pub fn editable(&self) -> bool {
        !self.read_only && matches!(self.lives, Lives::Mine | Lives::OnlyHere | Lives::New)
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

/// The numbers of the list's count line ("1,284 parts · 214 mine · 50 in this project").
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ListCounts {
    /// Every part the list finds.
    pub all: usize,
    /// Those of My parts.
    pub mine: usize,
    /// Those the project holds.
    pub project: usize,
}

/// How many parts Lately opened keeps.
const RECENT: usize = 5;

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
    /// The list's maker filter.
    pub maker: Option<String>,
    /// The list's rows, at most [`LIST_ROWS`].
    pub found: Vec<PartFound>,
    /// What the list holds before it is cut to [`LIST_ROWS`] (the count line): every part, those of My parts, those
    /// the project holds.
    pub counts: ListCounts,
    /// The makers of the parts the list finds without its maker filter, in their order: the Maker filter's choices.
    pub makers: Vec<String>,
    /// The parts opened lately, the latest first, by the copy each has or would have in the project (the list's group
    /// Lately opened).
    pub recent: Vec<Uuid>,
    /// The open part.
    pub sheet: Option<OpenSheet>,
    /// What New part… asks while no sheet is open.
    pub asking: Option<Asking>,
    /// The sheets waiting under a part made in passing (doc 07 §5.5: a cabinet's mounting plate made from its field),
    /// each with the field that takes the new part: Save returns to it with the part chosen, Back without it.
    pub under: Vec<(OpenSheet, String)>,
    /// Whether the list shows the Symbols shelf (doc 07 §5.3), else the parts.
    pub shelf_symbols: bool,
    /// The Symbols shelf's rows, Lately drawn first.
    pub symbols: Vec<SymbolFound>,
    /// Whether the place stepped aside for a test placement of the drawing on a page: the page shows, the place and
    /// its drawing wait, and Back to the drawing returns.
    pub aside: bool,
    /// The set an import brought (doc 07 §5.9), shown in the list as the scope This import until it is saved or
    /// discarded.
    pub set: Option<Box<ImportSet>>,
    /// Several parts selected in the list, shown on one sheet of the fields they share (doc 07 §5.9).
    pub many: Option<Box<super::parts_many::ManySheet>>,
    /// The list's groups folded shut, by their ids ([`ListGroup::id`]).
    pub folded: std::collections::BTreeSet<String>,
}

/// A group of the list (doc 07 §5.3): Lately opened, then the parts by what they are.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ListGroup {
    /// Lately opened.
    Recent,
    /// The parts of a kind, as the file spells it.
    Kind(String),
}

impl ListGroup {
    /// The group's id, which `parts.find {fold}` names: `recent`, or the kind.
    pub fn id(&self) -> &str {
        match self {
            ListGroup::Recent => "recent",
            ListGroup::Kind(k) => k,
        }
    }
}

/// The set an import brought into Parts and its first step (doc 07 §5.9): a table's column map, the rows read through
/// it, the row shown as it will be.
#[derive(Clone, PartialEq, Debug)]
pub struct ImportSet {
    /// The file's name (never its path).
    pub file: String,
    /// The set; `None` while the table's map is not usable (the map's refusal under the mapping says why).
    pub set: Option<ew_api::parts_set::PartsSet>,
    /// A table's column map; `None` for an EPLAN parts file.
    pub map: Option<super::parts::PartsMap>,
    /// The file as it was read, to read again when the map changes.
    pub input: Option<ew_api::catalog::ImportInput>,
    /// The row shown as it will be, by its index among the rows.
    pub row: Option<usize>,
    /// The copies the open project holds of an EPLAN set's parts: how many, and how many of them would change.
    pub project: Option<(usize, usize)>,
    /// An EPLAN file read for the open project's copies, which their update lays over them.
    pub held: Option<ew_api::catalog::PartsImport>,
    /// The shown row's part as its sheet reads it: each field with a value, by its path, and its text.
    pub texts: Vec<(&'static str, String)>,
    /// Ready for of the shown row's part.
    pub ready: Vec<ew_api::part_fields::Ready>,
}

impl ImportSet {
    /// The row shown as it will be.
    pub fn shown(&self) -> Option<&ew_api::parts_set::SetRow> {
        self.set.as_ref()?.rows.get(self.row?)
    }

    /// How many of its parts Save writes into My parts: the new and the changed.
    pub fn to_save(&self) -> usize {
        self.set.as_ref().map_or(0, |s| {
            let (new, changed, _, _) = s.counts();
            new + changed
        })
    }
}

/// A row of the Symbols shelf: a symbol by its address, its name, what it is and where it lives.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolFound {
    /// What `parts.open {symbol}` takes: `library:<library>:<item>`, or `id:<copy>` of a symbol only in the project.
    pub address: String,
    /// Its name in the window's language.
    pub name: String,
    /// What it is, in words (where the browsers list it).
    pub what: String,
    /// Where it lives, in words: Mine, Starter, a library's name, Only in this project.
    pub lives: String,
    /// Whether it is his (My parts or another library he can write).
    pub mine: bool,
    /// Whether he drew it lately (the shelf's group Lately drawn).
    pub recent: bool,
}

impl PartsPlace {
    /// The list's groups as the window shows them, each with its rows as indices into [`PartsPlace::found`] and whether
    /// it is folded: Lately opened, then a group per kind in the kinds' order.
    pub fn list_groups(&self) -> Vec<(ListGroup, Vec<usize>, bool)> {
        let mut out = Vec::new();
        let recent: Vec<usize> = (self.recent.iter())
            .filter_map(|c| self.found.iter().position(|f| f.copy == *c))
            .collect();
        if !recent.is_empty() {
            out.push((ListGroup::Recent, recent));
        }
        let mut kinds: Vec<&str> = self.found.iter().map(|f| f.offer.kind.as_str()).collect();
        kinds.sort_unstable();
        kinds.dedup();
        for k in kinds {
            let rows = (0..self.found.len())
                .filter(|i| self.found[*i].offer.kind == k)
                .collect();
            out.push((ListGroup::Kind(k.to_owned()), rows));
        }
        out.into_iter()
            .map(|(g, rows)| {
                let shut = self.folded.contains(g.id());
                (g, rows, shut)
            })
            .collect()
    }

    /// The parts as the list shows them, from its top, as `parts.open` names them: the rows of the groups that are
    /// open, a part that stands in two groups once, where it shows first.
    pub fn shown_order(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, rows, shut) in self.list_groups() {
            if shut {
                continue;
            }
            for i in rows {
                let p = self.found[i].param();
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
        out
    }

    /// The strip's sentence over the sheet: where the person came from with a purpose; `None` from the project's window.
    pub fn strip(&self) -> Option<String> {
        if let Some((under, field)) = self.under.last() {
            let word = ew_api::part_fields::field(field)
                .map(|f| strings::render(&ew_api::part_fields::word_in(f.word)))
                .unwrap_or_default();
            return Some(strings::format(
                key!("parts-strip-in-passing"),
                &[&under.title(), &word],
            ));
        }
        match &self.origin {
            Origin::Project | Origin::Station | Origin::Browser => None,
            Origin::Finding { page: Some(page) } => {
                Some(strings::format(key!("parts-strip-finding-page"), &[page]))
            }
            Origin::Finding { page: None } => Some(strings::text(key!("parts-strip-finding")).to_owned()),
            Origin::Chooser(c) => Some(strings::format(key!("parts-strip-choosing"), &[&c.target_text])),
            Origin::Form(f) => Some(strings::format(
                key!("parts-strip-form"),
                &[strings::text(f.title_key)],
            )),
        }
    }

    /// The label of the way back: Back without choosing, Back to the form, Back.
    pub fn back_label(&self) -> &'static str {
        if !self.under.is_empty() {
            return strings::text(key!("parts-back-without-it"));
        }
        match &self.origin {
            Origin::Project | Origin::Station | Origin::Browser => strings::text(key!("parts-back")),
            Origin::Finding { .. } => strings::text(key!("parts-back-to-messages")),
            Origin::Chooser(_) => strings::text(key!("parts-back-without-choosing")),
            Origin::Form(_) => strings::text(key!("parts-back-without-it")),
        }
    }

    /// Whether the middle shows the set an import brought: it is there and no part is open.
    pub fn set_shown(&self) -> bool {
        self.sheet.is_none() && self.many.is_none() && self.set.is_some()
    }

    /// The label of Save: Save n to My parts for an import's set, Keep in my parts and save for several parts some of
    /// which are only in this project, Save and use for the chooser's target from a chooser, else Save.
    pub fn save_label(&self) -> String {
        if self.set_shown() {
            let n = self.set.as_deref().map_or(0, ImportSet::to_save);
            return strings::format(key!("parts-set-save"), &[&n.to_string()]);
        }
        if (self.many.as_deref()).is_some_and(|m| m.sheets.iter().any(|(_, l)| *l == Lives::OnlyHere)) {
            return strings::text(key!("parts-save-keep")).to_owned();
        }
        match &self.origin {
            Origin::Chooser(c) => strings::format(key!("parts-save-use"), &[&c.target_text]),
            _ => strings::text(key!("parts-save")).to_owned(),
        }
    }
}

/// A part named by `parts.open`'s `part`: a copy the project holds, or a library's part.
pub(super) enum Named {
    Held(Uuid),
    Library(Uuid, Uuid),
}

impl ShellModel {
    /// The place, when it is shown.
    pub fn parts_place(&self) -> Option<&PartsPlace> {
        self.parts.as_deref()
    }

    /// Whether the place shows the set an import brought ([`PartsPlace::set_shown`]).
    pub(crate) fn set_shown(&self) -> bool {
        self.parts.as_deref().is_some_and(PartsPlace::set_shown)
    }

    /// Whether the place works on the libraries alone: it was opened with no project (from the start page), so a new
    /// part is saved into My parts only and nothing is kept in or taken out of a project.
    pub fn parts_without_project(&self) -> bool {
        self.parts_alone.is_some()
    }

    /// The session the place works on: the libraries alone where it was opened with no project, else the project's.
    /// [`ShellModel::parts_session_mut`] is the same, to change.
    pub(crate) fn parts_session(&self) -> Option<&ew_api::Session> {
        match &self.parts_alone {
            Some(alone) => Some(alone),
            None => self.project.as_ref().map(|p| &p.session),
        }
    }

    /// [`ShellModel::parts_session`] to change, with the place beside it.
    pub(crate) fn parts_session_mut(&mut self) -> (Option<&mut ew_api::Session>, Option<&mut PartsPlace>) {
        let session = match &mut self.parts_alone {
            Some(alone) => Some(&mut **alone),
            None => self.project.as_mut().map(|p| &mut p.session),
        };
        (session, self.parts.as_deref_mut())
    }

    /// The part `parts.open`'s `part` names: `id:<copy>`, `lib:<library>/<item>`, or a part number the project holds;
    /// without it, the Libraries navigator's part row when it has the focus, else the property grid's device's first
    /// part use, else that navigator's cursor row.
    pub(super) fn named_part(&self, p: &Map<String, Value>) -> Result<Option<Named>, Outcome> {
        let Some(session) = self.parts_session() else {
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
            let held = session.part_offers(&PartOfferQuery {
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

    /// The place shown over the project, from where the person is now: the part dialog (the strip names its target)
    /// or a form that needs a part; it keeps its list when it is shown already.
    pub(crate) fn enter_place(&mut self) {
        if let Some(place) = self.parts.as_deref_mut() {
            place.aside = false;
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
        // With no project the place works on the libraries alone, read anew at each opening.
        self.parts_alone =
            (!self.has_project()).then(|| Box::new(ew_api::Session::libraries_only(self.session_options())));
        self.parts = Some(Box::new(PartsPlace {
            origin,
            query: String::new(),
            scope: PartScope::All,
            kind: None,
            lacking: false,
            maker: None,
            found: Vec::new(),
            counts: ListCounts::default(),
            makers: Vec::new(),
            recent: Vec::new(),
            under: Vec::new(),
            sheet: None,
            asking: None,
            shelf_symbols: false,
            symbols: Vec::new(),
            aside: false,
            set: None,
            many: None,
            folded: std::collections::BTreeSet::new(),
        }));
        self.parts_refresh();
    }

    /// The place steps aside for the page (a test placement of the drawing): the page shows, the place and its drawing
    /// wait.
    pub(crate) fn step_aside(&mut self) {
        if let Some(place) = self.parts.as_deref_mut() {
            place.aside = true;
        }
    }

    /// The list again, from the place's query.
    pub(crate) fn parts_refresh(&mut self) {
        // The Symbols shelf, while it shows.
        if let Some((query, scope)) = (self.parts.as_deref())
            .filter(|p| p.shelf_symbols)
            .map(|p| (p.query.clone(), p.scope))
        {
            let found = self.symbols_found(&query, scope);
            if let Some(place) = self.parts.as_deref_mut() {
                place.symbols = found;
            }
        }
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return;
        };
        let session = &*session;
        let mut found = session.parts_find(
            &place.query,
            place.scope,
            place.kind.as_deref(),
            place.lacking,
            Some(strings::rule_language().to_owned()),
            usize::MAX,
        );
        // The set's makers on This import, whose rows are the set's.
        let set_makers = (place.scope == PartScope::Import)
            .then(|| place.set.as_deref().and_then(|s| s.set.as_ref()))
            .flatten()
            .map(|s| {
                s.rows
                    .iter()
                    .filter(|r| !r.maker.is_empty())
                    .map(|r| r.maker.clone())
            });
        let mut makers: Vec<String> = match set_makers {
            Some(m) => m.collect(),
            None => found.iter().map(|f| f.offer.manufacturer.clone()).collect(),
        };
        makers.sort_by_key(|m| m.to_lowercase());
        makers.dedup();
        place.makers = makers;
        if let Some(m) = &place.maker {
            found.retain(|f| &f.offer.manufacturer == m);
        }
        place.counts = ListCounts {
            all: found.len(),
            mine: found.iter().filter(|f| f.lives == Lives::Mine).count(),
            project: found
                .iter()
                .filter(|f| matches!(f.offer.pick, PartPick::Held(_)))
                .count(),
        };
        found.truncate(LIST_ROWS);
        place.found = found;
        if let Some(open) = &mut place.sheet
            && session.sheet_reads(&open.sheet)
        {
            open.lacks = session.sheet_lacks(&open.sheet);
            open.ready = session.sheet_ready(&open.sheet);
        }
    }

    /// Opens `sheet` in the place, where it lives.
    pub(crate) fn open_sheet(&mut self, sheet: PartSheet, bus_part: Option<BusPart>) {
        let Some(session) = self.parts_session() else {
            return;
        };
        let lives = session.sheet_lives(&sheet);
        let mine_too = lives == Lives::OnlyHere && sheet.source.is_some_and(|(_, item)| session.in_my_parts(item));
        let missing = missing_words(&session.names_missing(&session.sheet_part(&sheet)));
        let lacks = session.sheet_lacks(&sheet);
        let ready = session.sheet_ready(&sheet);
        let used_by = if sheet.new {
            Vec::new()
        } else {
            session.part_users(sheet.copy)
        };
        let manufacturers = session.part_manufacturers();
        let piece_symbols = piece_symbols_of(session, &sheet);
        // The libraries alone are his to write; a project only where it can be written.
        let read_only = !self.parts_without_project() && !self.is_writable();
        self.parts_opened = self.parts_opened.wrapping_add(1);
        let opened = self.parts_opened;
        if let Some(place) = self.parts.as_deref_mut() {
            place.asking = None;
            place.many = None;
            if !sheet.new {
                place.recent.retain(|r| *r != sheet.copy);
                place.recent.insert(0, sheet.copy);
                place.recent.truncate(RECENT);
            }
            place.sheet = Some(OpenSheet {
                opened,
                sheet,
                lives,
                mine_too,
                missing,
                lacks,
                ready,
                used_by,
                manufacturers,
                piece_symbols,
                error: None,
                bus_part,
                group: None,
                field: None,
                changed: false,
                read_only,
            });
        }
    }

    /// `parts.open {part?, field?}` (Parts, Edit part…, Open part): shows the place, on the part named (see
    /// [`ShellModel::named_part`]) when there is one, else on its list; with `field` (a field's path, as an outcome's
    /// part lack names it), at that field, ringed.
    pub(crate) fn act_parts_open(&mut self, p: &Map<String, Value>) -> Outcome {
        // A row of the set an import brought, shown as it will be.
        if let Some(v) = p.get("row") {
            let Some(row) = v.as_u64().and_then(|r| usize::try_from(r).ok()) else {
                return failed(key!("error-bad-param"), &["row"]);
            };
            let Some(shown) = self.parts.as_deref_mut().and_then(|p| p.set.as_deref_mut()) else {
                return Outcome::refused(key!("reason-no-import-set"));
            };
            if shown.set.as_ref().is_none_or(|s| row >= s.rows.len()) {
                return failed(key!("error-bad-param"), &["row"]);
            }
            shown.row = Some(row);
            self.set_row_shown();
            return Outcome::Done;
        }
        // A row taken into or out of a selection of several (Ctrl), or the rows up to it (Shift).
        if (flag(p, "add") || flag(p, "range")) && self.parts.is_some() && p.contains_key("part") {
            // The part as the list names it, a number the project holds too.
            let part = match self.named_part(p) {
                Ok(Some(Named::Held(c))) => format!("id:{c}"),
                Ok(Some(Named::Library(l, i))) => format!("lib:{l}/{i}"),
                Ok(None) => return failed(key!("error-bad-param"), &["part"]),
                Err(out) => return out,
            };
            return self.select_many(&part, flag(p, "range"));
        }
        if self.many_sheet().is_some_and(|m| m.changed) {
            return Outcome::refused(key!("reason-parts-unsaved"));
        }
        if self.parts_session().is_none() {
            // No project and no place: the place over the libraries alone (one a closed project left goes).
            self.parts = None;
            self.enter_place();
        }
        // A symbol of the Symbols shelf, in the drawing.
        if let Some(address) = or_return!(string_param(p, "symbol")).map(str::to_owned) {
            if let Some(out) = self.in_hand_asks("parts.open", p) {
                return out;
            }
            return self.open_symbol(&address);
        }
        let named = match self.named_part(p) {
            Ok(n) => n,
            Err(e) => return e,
        };
        let field = match string_param(p, "field") {
            Ok(Some(path)) => match ew_api::part_fields::field(path) {
                Some(f) => Some(f.path),
                None => return failed(key!("error-bad-param"), &["field"]),
            },
            Ok(None) => None,
            Err(e) => return e,
        };
        let sheet = match (self.parts_session(), named) {
            (Some(session), Some(Named::Held(c))) => match session.part_draft(c) {
                Ok(s) => Some(s),
                Err(_) => return Outcome::refused(key!("reason-part-edit-target")),
            },
            (Some(session), Some(Named::Library(l, i))) => match session.library_part_draft(l, i) {
                Ok(s) => Some(s),
                Err(_) => return Outcome::refused(key!("reason-part-edit-target")),
            },
            _ => None,
        };
        if sheet.is_some()
            && let Some(out) = self.settle_part_card("parts.open", p)
        {
            return out;
        }
        // The package dialog's Edit part… of a lack line leads away from it, as its Workshop stock… does.
        if sheet.is_some() && matches!(self.dialog, Some(Dialog::Package(_))) {
            self.dialog = None;
        }
        self.enter_place();
        let like = self.parts.as_deref().and_then(|p| p.asking) == Some(Asking::LikeWhich);
        if let Some(sheet) = sheet {
            if like && let Some(session) = self.parts_session() {
                return match session.new_part_draft_copying(&sheet) {
                    Ok(mut new) => {
                        let _ = session.set_draft_field(&mut new, "part_number", "");
                        new.messages.clear();
                        self.open_sheet(new, None);
                        Outcome::Done
                    }
                    Err(e) => draft_refusal(e),
                };
            }
            self.open_sheet(sheet, None);
            if let Some(open) = self.parts.as_deref_mut().and_then(|p| p.sheet.as_mut()) {
                open.group = field.and_then(Group::of);
                open.field = field;
            }
        }
        Outcome::Done
    }

    /// Edit part… of an outcome that comes from a part's missing datum (a finding, a refusal): Parts on the part at the
    /// field, ringed; opened from a finding in Messages (`origin`), its strip says so and Back returns there.
    pub(crate) fn open_at_lack(&mut self, lack: PartLack, origin: Option<Origin>) -> Outcome {
        let shown = self.parts.is_some();
        let out = self.dispatch(
            "parts.open",
            &serde_json::json!({"part": format!("id:{}", lack.copy), "field": lack.field}),
        );
        if out == Outcome::Done
            && !shown
            && let (Some(origin), Some(place)) = (origin, self.parts.as_deref_mut())
        {
            place.origin = origin;
        }
        out
    }

    /// The outcome `out` of a commit that command error `e` refused: a refusal for a part's missing datum is said in a
    /// toast that offers Edit part… ([`ShellModel::say_lacking`]).
    pub(crate) fn command_refused(&mut self, e: &ew_api::CmdError, out: Outcome) -> Outcome {
        if let (Some(lack), Outcome::Failed { message }) = (e.part_lack(), &out) {
            self.say_lacking(message.clone(), lack);
        }
        out
    }

    /// A command refused for a part's missing datum `lack` (doc 07 §5.7): its `message` in a toast that offers Edit
    /// part…, which then is where the refusal is said (no banner beside it).
    fn say_lacking(&mut self, message: String, lack: PartLack) {
        let Some(epoch) = self.project.as_ref().map(|p| p.epoch) else {
            return;
        };
        let offer = crate::notifications::Offer::new(
            "edit_part",
            ew_api::checks::messages::lack_label(strings::rule_language()),
            crate::notifications::Validity::Session { epoch },
            crate::notifications::Run::Steps {
                steps: vec![(
                    "parts.open",
                    serde_json::json!({"part": format!("id:{}", lack.copy), "field": lack.field}),
                )],
                once: false,
                keeps: false,
            },
        );
        self.notes.push(
            crate::notifications::Note::new("refusal", message)
                .offering([Some(offer)])
                .toned(crate::notifications::NoteTone::Warning),
        );
        self.said_at(super::said::Said::Toast);
    }

    /// Page `page` in words as the page list names it: its folio and its title ("904 Door").
    pub(crate) fn page_words(&self, page: Uuid) -> Option<String> {
        let row = self.page_rows().iter().find(|r| r.id == page)?;
        Some(format!("{} {}", row.folio, row.title).trim().to_owned())
    }

    /// The card of the open part's Save that waits (what the Save changes in the project) belongs to that part: before
    /// `action` opens another part, its `card` applies the card (`apply`) or cancels it (`cancel`); without it the
    /// window asks Save, Discard or Stay and a script is refused in words. `None` where `action` goes on.
    fn settle_part_card(&mut self, action: &'static str, p: &Map<String, Value>) -> Option<Outcome> {
        if !matches!(self.props.card().map(|c| &c.origin), Some(CardOrigin::Part(_))) {
            return None;
        }
        let out = match string_param(p, "card") {
            Err(e) => e,
            Ok(Some("apply")) => self.act_preview_apply(),
            Ok(Some("cancel")) => self.act_preview_cancel(),
            Ok(Some(_)) => failed(key!("error-bad-param"), &["card"]),
            Ok(None) => match self.mode {
                crate::shell::Mode::Headless => Outcome::refused(key!("reason-parts-card-waits")),
                crate::shell::Mode::Gui => {
                    let number = (self.parts.as_deref().and_then(|p| p.sheet.as_ref()))
                        .map_or_else(String::new, OpenSheet::title);
                    let next = Some((action, p.clone()));
                    return Some(self.open_dialog(Dialog::ConfirmLeavePart { number, next }));
                }
            },
        };
        if out != Outcome::Done {
            return Some(out);
        }
        if matches!(&self.dialog, Some(Dialog::ConfirmLeavePart { .. })) {
            self.dialog = None;
        }
        None
    }

    /// Values typed into the sheet of several parts and not saved, before `action` opens another part: `card` `apply`
    /// saves them first (where the project holds them their card then waits, and the action stops there), `cancel`
    /// drops them; without it the window asks Save, Discard or Stay and a script is refused in words. `None` where
    /// `action` goes on.
    fn settle_many(&mut self, action: &'static str, p: &Map<String, Value>) -> Option<Outcome> {
        let many = self.many_sheet().filter(|m| m.changed)?;
        let numbers: Vec<String> = many.numbers().into_iter().map(|(n, _)| n).collect();
        match string_param(p, "card") {
            Err(e) => Some(e),
            Ok(Some("apply")) => {
                self.dialog = None;
                let out = self.save_many();
                let waits = matches!(self.props.card().map(|c| &c.origin), Some(CardOrigin::Part(_)));
                (out != Outcome::Done || waits).then_some(out)
            }
            Ok(Some("cancel")) => {
                self.dialog = None;
                if let Some(place) = self.parts.as_deref_mut() {
                    place.many = None;
                }
                None
            }
            Ok(Some(_)) => Some(failed(key!("error-bad-param"), &["card"])),
            Ok(None) => Some(match self.mode {
                crate::shell::Mode::Headless => Outcome::refused(key!("reason-parts-unsaved")),
                crate::shell::Mode::Gui => self.open_dialog(Dialog::ConfirmLeavePart {
                    number: numbers.join(", "),
                    next: Some((action, p.clone())),
                }),
            }),
        }
    }

    /// `parts.find {text?, scope?, kind?, maker?, lacking?, shelf?}`: the list's search, scope (`all`, `project`,
    /// `mine`), kind filter (`all` for none), maker, Lacks something, and the shelf it shows (`parts`, `symbols`).
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
        let maker = or_return!(string_param(p, "maker")).map(str::to_owned);
        let fold = or_return!(string_param(p, "fold")).map(str::to_owned);
        let shelf = match or_return!(string_param(p, "shelf")) {
            None => None,
            Some("parts") => Some(false),
            Some("symbols") => Some(true),
            Some(_) => return failed(key!("error-bad-param"), &["shelf"]),
        };
        // The Symbols shelf keeps the scope he used last.
        if (shelf == Some(true) || self.parts.as_deref().is_some_and(|p| p.shelf_symbols))
            && let Some(s) = scope
        {
            self.settings.symbols_scope = Some(s.word().to_owned());
            self.save_settings();
        }
        let last_scope = (self.settings.symbols_scope.as_deref())
            .and_then(|w| PartScope::ALL.into_iter().find(|s| s.word() == w));
        let Some(place) = self.parts.as_deref_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        if let Some(symbols) = shelf
            && symbols != place.shelf_symbols
        {
            place.shelf_symbols = symbols;
            if symbols && let Some(s) = last_scope {
                place.scope = s;
            }
        }
        if let Some(t) = text {
            place.query = t;
        }
        if let Some(s) = scope {
            // This import shows the set and its first step in the middle, the open part set aside unless it has
            // changes.
            if s == PartScope::Import {
                if place.set.is_none() {
                    return Outcome::refused(key!("reason-no-import-set"));
                }
                if place.sheet.as_ref().is_some_and(|o| o.changed) {
                    return Outcome::refused(key!("reason-parts-unsaved"));
                }
                place.sheet = None;
                place.shelf_symbols = false;
            }
            place.scope = s;
        }
        if let Some(k) = kind {
            place.kind = (k != "all").then_some(k);
        }
        if let Some(m) = maker {
            place.maker = (m != "all").then_some(m);
        }
        if let Some(v) = p.get("lacking") {
            place.lacking = v.as_bool().unwrap_or(false);
        }
        // A group of the list folded shut, or opened again.
        if let Some(id) = fold
            && !place.folded.remove(&id)
        {
            place.folded.insert(id);
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
        let for_field = or_return!(string_param(p, "for_field")).map(str::to_owned);
        if let Some(out) = self.settle_part_card("parts.new", p) {
            return out;
        }
        if let Some(out) = self.settle_many("parts.new", p) {
            return out;
        }
        let Some(session) = self.parts_session() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let open = self.parts.as_deref().and_then(|p| p.sheet.as_ref());
        let made = match (&symbol, start, like) {
            (Some(address), _, _) => session.new_part_draft_for_symbol(address),
            (None, Some(start), _) => session.start_sheet(start),
            (None, None, true) if open.is_some() => {
                let held = &open.expect("checked").sheet;
                session.new_part_draft_copying(held).map(|mut d| {
                    let _ = session.set_draft_field(&mut d, "part_number", "");
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
            let _ = session.set_draft_field(sheet, f, v);
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
        // A part made in passing: the open sheet waits under it with the field that takes it.
        if let Some(field) = for_field
            && let Some(place) = self.parts.as_deref_mut()
            && let Some(waiting) = place.sheet.take()
        {
            place.under.push((waiting, field));
        }
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
    pub(crate) fn sheet_changed(&mut self) {
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return;
        };
        if let Some(open) = place.sheet.as_mut() {
            open.changed = true;
            open.error = None;
            // While a field's text does not read, or a table row is not filled in yet, Ready for stays as it was: it
            // says what the part lacks, not what the part as it was opened lacks (and the sheet does not jump).
            if session.sheet_reads(&open.sheet) {
                open.lacks = session.sheet_lacks(&open.sheet);
                open.ready = session.sheet_ready(&open.sheet);
                open.missing = missing_words(&session.names_missing(&session.sheet_part(&open.sheet)));
            }
            open.piece_symbols = piece_symbols_of(session, &open.sheet);
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
        // Several parts selected: the value goes to all of them.
        if self.many_sheet().is_some() {
            return self.many_field(&field, &value);
        }
        // The drawing's short sheet: its name and what it is.
        if let Some(key) = field.strip_prefix("symbol.")
            && self.symbol_editor().is_some()
        {
            return match self.set_symbol_field(key, &value) {
                Ok(()) => Outcome::Done,
                Err(message) => Outcome::Failed { message },
            };
        }
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let session = &*session;
        let Some(open) = place.sheet.as_mut() else {
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
                    session.set_draft_field(&mut open.sheet, &field, &value)
            {
                return failed(key!("error-part-field"), &[&field]);
            }
            if field.starts_with("plug.") || field.starts_with("bus.") {
                let words = plug_description(&open.sheet, plug);
                let _ = session.set_draft_field(&mut open.sheet, "description", &words);
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
        let out = match session.set_draft_field(&mut open.sheet, &field, &value) {
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
    /// `parts.row {group}` adds a group of the contents list's offer (`ports`, `plc`: `PartSheet::groups_to_add`),
    /// which the sheet then shows empty.
    pub(crate) fn act_parts_row(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(name) = or_return!(string_param(p, "group")) {
            let Some(open) = self.open_mut() else {
                return Outcome::refused(key!("reason-no-part-open"));
            };
            if !open.editable() {
                return Outcome::refused(key!("reason-part-not-yours"));
            }
            let Some(group) =
                ew_api::part_fields::Group::named(name).filter(|g| open.sheet.groups_to_add().contains(g))
            else {
                return failed(key!("error-bad-param"), &["group"]);
            };
            open.sheet.added.push(group);
            return Outcome::Done;
        }
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
        } else if table == "pieces" {
            match remove {
                None => {
                    open.sheet.add_piece();
                    true
                }
                Some(i) => open.sheet.remove_piece(i),
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
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let Some(open) = place.sheet.as_mut() else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        match session.draft_outline_import(&mut open.sheet, &path) {
            Ok(_) => {
                self.sheet_changed();
                Outcome::Done
            }
            Err(why) => {
                let message = strings::render(&why);
                open.error = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    /// `parts.discard`: the open part as it was opened (a new part's sheet closes).
    pub(crate) fn act_parts_discard(&mut self) -> Outcome {
        if self.symbol_editor().is_some() {
            return self.discard_drawing();
        }
        if self.set_shown() {
            return self.discard_set();
        }
        if self.many_sheet().is_some() {
            return self.reopen_many();
        }
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
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let again = match source {
            Some((l, i)) => session.library_part_draft(l, i),
            None => session.part_draft(copy),
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
        if self.symbol_editor().is_some() {
            return self.own_drawing();
        }
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::refused(key!("reason-no-part-open"));
        };
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let mut sheet = match session.new_part_draft_copying(&open.sheet) {
            Ok(s) => s,
            Err(e) => return draft_refusal(e),
        };
        for field in ["part_number", "manufacturer"] {
            let _ = session.set_draft_field(&mut sheet, field, "");
        }
        sheet.messages.clear();
        self.open_sheet(sheet, None);
        Outcome::Done
    }

    /// `parts.keep` (Keep in my parts): the open part, only in this project, written into My parts with the parts only
    /// in this project that it names, and the project's devices given those parts of his (one undo step); the toast
    /// names the parts kept with it.
    pub(crate) fn act_parts_keep(&mut self) -> Outcome {
        let Some((copy, title)) = self
            .parts
            .as_deref()
            .and_then(|p| p.sheet.as_ref())
            .filter(|o| o.lives == Lives::OnlyHere)
            .map(|o| (o.sheet.copy, o.title()))
        else {
            return Outcome::refused(key!("reason-part-keep"));
        };
        let (Some(session), _) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let kept = match session.keep_in_my_parts(&[copy]) {
            Ok(k) => k,
            Err(e) => return self.save_refused(e),
        };
        let out = self.run_commit(&kept.command);
        if out == Outcome::Done {
            self.libraries_changed();
            let to = kept.command.params["to"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok());
            if let (Some(session), Some(to)) = (self.parts_session(), to)
                && let Ok(sheet) = session.part_draft(to)
            {
                self.open_sheet(sheet, None);
            }
            self.parts_refresh();
            let toast = if kept.with.is_empty() {
                strings::format(key!("toast-part-kept"), &[&title])
            } else {
                strings::format(key!("toast-part-kept-with"), &[&title, &kept.with.join(", ")])
            };
            self.notify(toast, true);
        }
        out
    }

    /// `parts.back {save?, discard?, show?}`: returns to where the person came from: the part dialog with the open part
    /// under its cursor, the form that asked for a part, else the project's window; with `show`, that device on its page
    /// (the heading's "-102Q1 uses it"). While the open part has changes not saved the window asks Save, Discard or
    /// Stay first (a script is refused in words); `save` saves and then leaves, `discard` leaves without them.
    pub(crate) fn act_parts_back(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.symbol_editor().is_some() {
            return self.back_from_drawing();
        }
        let Some(place) = self.parts.as_deref() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        // What was typed and not saved: the open part's, or the parts' selected together (named by their numbers).
        let unsaved = match (place.sheet.as_ref(), place.many.as_deref()) {
            (Some(open), _) if open.changed => Some(open.title()),
            (_, Some(many)) if many.changed => Some(
                many.numbers()
                    .into_iter()
                    .map(|(n, _)| n)
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            _ => None,
        };
        if let Some(number) = unsaved {
            if flag(p, "save") {
                // Save, then leave once it saved; a refusal stays on the sheet, said under it.
                self.dialog = None;
                let out = self.act_parts_save(&Map::new());
                if out != Outcome::Done
                    || self
                        .parts
                        .as_deref()
                        .and_then(|p| p.sheet.as_ref())
                        .is_some_and(|o| o.changed && o.sheet.new)
                {
                    return out;
                }
                if self.parts.is_none() {
                    // Save and use for, or a form's part: the place gave way already.
                    return Outcome::Done;
                }
            } else if !flag(p, "discard") {
                return match self.mode {
                    crate::shell::Mode::Headless => Outcome::refused(key!("reason-parts-unsaved")),
                    crate::shell::Mode::Gui => {
                        self.open_dialog(Dialog::ConfirmLeavePart { number, next: None })
                    }
                };
            }
        }
        self.dialog = None;
        if let Some((waiting, field)) = self.parts.as_deref_mut().and_then(|p| p.under.pop()) {
            return self.back_to_waiting(waiting, &field, None);
        }
        let show = string_param(p, "show").ok().flatten().map(str::to_owned);
        let place = *self.parts.take().expect("checked");
        self.parts_alone = None;
        // A drawing in hand stays in hand, not shown.
        if let Some(editor) = &mut self.symbol_editor {
            editor.shown = false;
        }
        match place.origin {
            Origin::Project | Origin::Station | Origin::Browser | Origin::Finding { .. } => {}
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
        // The heading's "-102Q1 uses it": the device on its page.
        if let Some(device) = show {
            return self.act_nav_show(&Map::from_iter([("record".to_owned(), Value::String(device))]));
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
                        crate::shell::Mode::Gui => self.open_dialog(Dialog::ConfirmRemovePart { number }),
                    };
                }
                let (Some(session), _) = self.parts_session_mut() else {
                    return Outcome::refused(key!("reason-no-parts-place"));
                };
                let removed = match session.remove_own_part(library, item) {
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
        let session = self.parts_session()?;
        let open = self.parts.as_deref()?.sheet.as_ref()?;
        let held = {
            let q = session.query();
            copy.to_string().parse().ok().and_then(|id| {
                q.parts()
                    .get(id)
                    .map(|c| (c.library_id.as_uuid(), c.item_id.as_uuid()))
            })
        };
        let (library, item) = held.or(open.sheet.source)?;
        session.own_part_number(library, item)?;
        Some((library, item))
    }

    /// `parts.save {another?}` (Save, Save and use for): saves the open part where it lives. A new part goes into My
    /// parts and the project at once (from the part dialog it is then given to the devices it was chosen for; from a
    /// form, the form is back with it chosen; with `another`, a new cable type's next size opens); a part of My parts
    /// writes its next version there and opens the card of what changes for the project; a part only in this project
    /// opens the card of its correction. A field that does not parse refuses at its field; a part that is not his is
    /// refused in words.
    pub(crate) fn act_parts_save(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.symbol_editor().is_some() {
            return self.save_drawing();
        }
        if self.set_shown() {
            return self.save_set();
        }
        if self.many_sheet().is_some() {
            return self.save_many();
        }
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
        // A row's cell left empty is named by its column's word ("Name: Row 1 needs a value here…").
        let column = match &e {
            PartDraftError::Field(m) if m.key == "error-part-cell-needed" => {
                Some(super::part_dialog::field_label(&m.field))
            }
            _ => None,
        };
        let out = draft_refusal(e);
        let words = match &out {
            Outcome::Refused { reason } => strings::render(reason),
            Outcome::Failed { message } => message.clone(),
            Outcome::Done => String::new(),
        };
        open.error = Some(match column {
            Some(c) => strings::format(key!("part-form-message"), &[&c, &words]),
            None => words,
        });
        out
    }

    /// Save of a part of My parts ([`ShellModel::save_own`]): where the project holds it, the card of what changes
    /// opens beside the sheet and its Apply writes My parts and commits the step; else its next version is written at
    /// once, with the parts only in this project that it names.
    fn parts_save_mine(&mut self) -> Outcome {
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::Done;
        };
        let (title, sheet) = (open.title(), open.sheet.clone());
        let said = strings::format(key!("parts-saved-mine"), &[&title]);
        match self.save_own(&[sheet], &title, said) {
            Ok(out) => {
                // Saved now; else the card waits, and the part stays changed until its Apply.
                if self.parts_save.is_none()
                    && let Some(open) = self.open_mut()
                {
                    open.changed = false;
                }
                out
            }
            Err((_, e)) => self.save_refused(e),
        }
    }

    /// Save of a part only in this project: the card of its correction (`part.edit_copy`, in place) opens beside the
    /// sheet; Apply commits it.
    fn parts_save_here(&mut self) -> Outcome {
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let Some(open) = self.parts.as_deref().and_then(|p| p.sheet.as_ref()) else {
            return Outcome::Done;
        };
        let title = open.title();
        let plan = match session.part_edit_plan(&open.sheet) {
            Ok(plan) => plan,
            Err(e) => return self.save_refused(e),
        };
        let devices: Vec<String> = plan.devices.iter().map(|d| self.tag_of(d.as_uuid())).collect();
        let card = PartCard {
            title,
            fields: card_fields(Some(session), &plan.fields),
            devices,
            removed: plan.bom_removed.clone(),
            added: plan.bom_added.clone(),
            changed: plan.bom_changed.clone(),
            lost: plan.lost.clone(),
            boxes: [
                self.box_words(&plan.boxes),
                self.symbol_words(&plan.symbols),
                self.own_symbol_words(&plan.own_symbols),
            ]
            .concat(),
            kept: Vec::new(),
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
        let alone = self.parts_without_project();
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return Outcome::Done;
        };
        let Some(open) = place.sheet.as_ref() else {
            return Outcome::Done;
        };
        let item = open.sheet.copy;
        // With no project the part goes into My parts alone (the library's ID kept to show it from there); else into
        // My parts and the project by the command.
        let made = if alone {
            (session.new_part_in_my_parts(&open.sheet))
                .map(|(lib, name)| (None, Some(name), Some(lib), Vec::new()))
        } else {
            (session.new_part_command(&open.sheet))
                .map(|made| (Some(made.command), made.library, None, made.with))
        };
        let (command, library, mine, with) = match made {
            Ok(made) => made,
            Err(e) => return self.save_refused(e),
        };
        let out = command.as_ref().map_or(Outcome::Done, |c| self.run_commit(c));
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
        let toast = match &library {
            // With no project open it is in My parts alone.
            Some(_) if alone => strings::format(key!("parts-saved-mine"), &[&number]),
            Some(name) if !with.is_empty() => strings::format(
                key!("toast-part-new-library-with"),
                &[&number, name, &with.join(", ")],
            ),
            Some(name) => strings::format(key!("toast-part-new-library"), &[&number, name]),
            None => strings::format(key!("toast-part-new"), &[&number]),
        };
        self.notify(toast, true);
        self.libraries_changed();
        if another {
            return self.parts_new_like(open);
        }
        if let Some((waiting, field)) = self.parts.as_deref_mut().and_then(|p| p.under.pop()) {
            return self.back_to_waiting(waiting, &field, Some(&number));
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
            Origin::Project | Origin::Finding { .. } => {
                let saved = self.parts_session().and_then(|session| match mine {
                    Some(lib) => session.library_part_draft(lib, item).ok(),
                    None => {
                        let q = session.query();
                        let copy = (q.parts().iter())
                            .find(|(_, c)| c.item.part_number.as_str() == number)
                            .map(|(id, _)| id.as_uuid())?;
                        session.part_draft(copy).ok()
                    }
                });
                if let Some(sheet) = saved {
                    self.open_sheet(sheet, None);
                }
                self.parts_refresh();
                Outcome::Done
            }
        }
    }

    /// The sheet that waited under a part made in passing, open again: its field `field` takes the part numbered
    /// `number` when it was saved.
    fn back_to_waiting(&mut self, waiting: OpenSheet, field: &str, number: Option<&str>) -> Outcome {
        if let Some(place) = self.parts.as_deref_mut() {
            place.sheet = Some(waiting);
        }
        if let Some(number) = number {
            let _ = self.act_parts_field(&Map::from_iter([
                ("field".to_owned(), Value::String(field.to_owned())),
                ("value".to_owned(), Value::String(number.to_owned())),
            ]));
        }
        self.parts_refresh();
        Outcome::Done
    }

    /// A new part like the saved `saved` but for its number and its cross-section (Save and add another size): its own
    /// item ID, the form's way back kept.
    fn parts_new_like(&mut self, saved: OpenSheet) -> Outcome {
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let mut sheet = saved.sheet.clone();
        sheet.copy = match session.new_part_draft() {
            Ok(d) => d.copy,
            Err(e) => return draft_refusal(e),
        };
        for field in ["part_number", "cable.csa"] {
            let _ = session.set_draft_field(&mut sheet, field, "");
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
        let session = self.parts_session()?;
        // A part drawn as a box: its box with its pins, as the page lays it out; else its pieces' symbols.
        sheet
            .box_picture()
            .or_else(|| session.part_schematic(&session.sheet_part(sheet)))
    }

    /// The open sheet's part "On a bus page" as its texts give it now (doc 10 §2.7): the bus symbol a bus page draws its
    /// devices with, his own or the program's; `None` for a part without network ports.
    pub fn part_bus_symbol(&self, sheet: &PartSheet) -> Option<ew_api::output::PartBusSymbol> {
        let session = self.parts_session()?;
        session.part_bus_symbol(&session.sheet_part(sheet))
    }
}
