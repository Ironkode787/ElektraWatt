// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The doors of Parts (doc 07 §3.6, §5.9): `parts.import {path?}`, Import… of the place, goes on by what the file is.
//!
//! - **A table** (CSV or XLSX): its columns are mapped to the part's declared fields as the set's first step, the map
//!   pre-filled from the one saved for the file's source key (`<config dir>/imports/<key>.toml`), else from the headers
//!   that name a field by its words; `parts.map` changes the source key or a column's field, and the set follows at once.
//! - **An EPLAN parts file** (an EDZ, a ZIP or 7-Zip of one, a parts XML): the Data Portal terms notice first, once per
//!   session, for an EDZ; the file is read as a job.
//! - **An outline drawing** (DXF) on the Parts shelf: the open part's front outline (`parts.outline`).
//! - **A symbol file** on the Symbols shelf (a QElectroTech element, an EPLAN symbol file, a DXF drawing, a symbol's own
//!   JSON): its first drawing in the drawing, the file's others beside it, `index` taking another.
//!
//! What a table or a parts file brings is the set ([`ew_api::parts_set::PartsSet`]): the list's scope This import, each
//! row new, changed, the same or refused with why; Save writes its new and changed parts into My parts (the map saved
//! for the source then). With an EPLAN parts file, the copies the open project holds of its parts are offered an update
//! (`parts.import {update}`, the previewed `library.update`). File ▸ Import… goes on to here for a file of parts.
//!
//! The EPLAN report's column mapping card (a report becomes a project, no part) is the other user of the column map.

use std::path::PathBuf;

use ew_api::catalog;
use ew_api::eplan::{self, ReportMap, ReportTarget};
use ew_api::parts_set::{PartsSet, SetRefusal, SetStatus};
use serde_json::{Map, Value};

use super::parts_place::ImportSet;
use super::read_jobs::Finish;
use super::{Dialog, EplanStep, Outcome, Prompt, ShellModel, failed, flag, string_param};
use crate::key;
use crate::strings;

/// One column of the mapping.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapColumn {
    /// The header.
    pub header: String,
    /// The first value under it.
    pub sample: String,
    /// The member it imports as (`part_number`, `mech.width`, `tech_data.i_n`); `None`: not imported.
    pub member: Option<String>,
}

/// What the column mapping maps for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    /// A parts table (the fields of a part): the set's first step in Parts.
    Parts,
    /// An EPLAN report spreadsheet of the given kind (FR-IMP-029): the members of that report.
    Report(ReportTarget),
}

/// The words of a report member (`strip`, `designation`, ...), `None` for a name that is no member.
fn report_member_key(member: &str) -> Option<crate::strings::Key<'static>> {
    Some(match member {
        "strip" => key!("eplan-member-strip"),
        "designation" => key!("eplan-member-designation"),
        "level" => key!("eplan-member-level"),
        "part" => key!("eplan-member-part"),
        "description" => key!("eplan-member-description"),
        "from" => key!("eplan-member-from"),
        "to" => key!("eplan-member-to"),
        "from_level" => key!("eplan-member-from-level"),
        "to_level" => key!("eplan-member-to-level"),
        _ => return None,
    })
}

/// The column mapping of a parts table (the set's first step in Parts) or of an EPLAN report (its card).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartsMap {
    /// What it maps for.
    pub kind: MapKind,
    /// The 1-based row of a report's header among the rows with text.
    header_row: u32,
    /// The table's file name (never its path).
    pub file: String,
    path: PathBuf,
    /// The source key the map is saved under.
    pub source: String,
    /// Whether the members came from the map saved for the source key.
    pub saved: bool,
    /// The columns in table order.
    pub columns: Vec<MapColumn>,
    /// Why the table is not read: the importer's message for the map (`the column map needs a column for
    /// `part_number``), or the last read's failure.
    pub error: Option<String>,
}

/// The identifier a header gives a technical value or a free property of its own (`Rated current` gives
/// `rated_current`); empty where it gives none.
fn header_ident(header: &str) -> String {
    let ident: String = header
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    ident.trim_matches('_').to_owned()
}

impl PartsMap {
    /// The member choices of the column `header` of a parts table: the part's fields a column goes to
    /// ([`ew_api::parts_set::field_columns`]), the description in each language of the window after the description,
    /// the column's own technical value and free property (`tech_data.<header as identifier>`) where the header gives an
    /// identifier the importer takes, and Like.
    pub fn choices(header: &str) -> Vec<String> {
        let mut out: Vec<String> = ew_api::parts_set::field_columns()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let at = out
            .iter()
            .position(|m| m == "description")
            .map_or(out.len(), |i| i + 1);
        for (i, lang) in crate::settings::LANGUAGES.iter().enumerate() {
            out.insert(at + i, format!("description.{lang}"));
        }
        let ident = header_ident(header);
        for head in ["tech_data", "free_props"] {
            let name = format!("{head}.{ident}");
            if !ident.is_empty() && !out.contains(&name) && catalog::Member::parse(&name).is_ok() {
                out.push(name);
            }
        }
        out.push("like".to_owned());
        out
    }

    /// The member choices of the column `header` for this mapping: a parts table's ([`PartsMap::choices`]), or the
    /// members of the report's kind.
    pub fn member_choices(&self, header: &str) -> Vec<String> {
        match self.kind {
            MapKind::Parts => Self::choices(header),
            MapKind::Report(target) => target.members().iter().map(|(m, _)| (*m).to_owned()).collect(),
        }
    }

    /// A member in words for this mapping ([`PartsMap::member_text`], or the report member's words).
    pub fn member_words(&self, member: &str, header: &str) -> String {
        match self.kind {
            MapKind::Parts => Self::member_text(member, header),
            MapKind::Report(_) => match report_member_key(member) {
                Some(k) => strings::text(k).to_owned(),
                None => member.to_owned(),
            },
        }
    }

    /// The report map of the chosen members, as the importer checks it (a required member without a column, a member
    /// of another kind of report).
    fn report_map(&self) -> Result<ReportMap, String> {
        let MapKind::Report(target) = self.kind else {
            return Err(String::new());
        };
        let mut columns = std::collections::BTreeMap::new();
        for c in &self.columns {
            if let Some(m) = &c.member
                && columns.insert(m.clone(), c.header.clone()).is_some()
            {
                return Err(strings::format(key!("parts-map-twice"), &[m]));
            }
        }
        let map = ReportMap {
            target,
            sheet: None,
            header_row: Some(self.header_row),
            columns,
        };
        ReportMap::from_toml(&map.to_toml()).map(|_| map)
    }

    /// The column map of the chosen members, or the importer's message why it is not usable.
    fn column_map(&self) -> Result<catalog::ColumnMap, String> {
        let mut chosen = std::collections::BTreeSet::new();
        if let Some(twice) = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref())
            .find(|m| !chosen.insert(*m))
        {
            return Err(strings::format(key!("parts-map-twice"), &[twice]));
        }
        let pairs = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref().map(|m| (m, c.header.as_str())));
        catalog::ColumnMap::new(None, pairs).map_err(|e| e.to_string())
    }

    /// Whether Import can run: the importer takes the map.
    pub fn can_import(&self) -> bool {
        match self.kind {
            MapKind::Parts => self.column_map().is_ok(),
            MapKind::Report(_) => self.report_map().is_ok(),
        }
    }

    /// A member in words, as the mapping shows it (doc 16a P2): a field by its group and its word ("Identity ▸ Maker"),
    /// a size several kinds share by its word ("Width"), a description in a language ("Identity ▸ Description,
    /// Nederlands"), a technical value or a free property of the column's own ("Technical data: Nennstrom"), Like; with
    /// `header` the column's header. A name that is no member is shown as it is. Describe text and scripts keep the
    /// member names.
    pub fn member_text(member: &str, header: &str) -> String {
        let in_group = |group: ew_api::part_fields::Group, word: &str| {
            strings::format(
                key!("parts-member-in-group"),
                &[strings::text(strings::Key::run_time(group.word())), word],
            )
        };
        match catalog::Member::parse(member) {
            Ok(catalog::Member::Like) => strings::text(key!("parts-member-like")).to_owned(),
            Ok(catalog::Member::Description(lang)) => {
                // The language named in the window's language ("Dutch", in Dutch "Nederlands").
                let name = strings::lookup(&format!("language-in-{lang}")).map_or_else(|| lang.clone(), str::to_owned);
                let word = Self::plain_word("description").unwrap_or_default();
                in_group(
                    ew_api::part_fields::Group::Identity,
                    &strings::format(key!("parts-member-description-lang"), &[&word, &name]),
                )
            }
            Ok(catalog::Member::FreeProp(_)) => strings::format(key!("parts-member-free-prop"), &[header]),
            // A cable's cores, as its Cores form names them.
            Ok(catalog::Member::Field(_)) if Self::cable_word(member).is_some() => {
                in_group(ew_api::part_fields::Group::Cable, Self::cable_word(member).unwrap_or_default())
            }
            Ok(catalog::Member::TechData(_)) | Ok(catalog::Member::Field(_)) => {
                match ew_api::part_fields::field(member) {
                    Some(f) => in_group(f.group, &strings::render(&ew_api::part_fields::word(f.word))),
                    None if member.starts_with("tech_data.") => {
                        strings::format(key!("parts-member-tech-data"), &[header])
                    }
                    // A size several kinds share: its word ("Width").
                    None => ew_api::part_fields::PART_FIELDS
                        .iter()
                        .find(|f| f.path.rsplit('.').next() == Some(member) && f.path.starts_with("mech."))
                        .map_or_else(
                            || member.to_owned(),
                            |f| strings::render(&ew_api::part_fields::word(f.word)),
                        ),
                }
            }
            Err(_) => member.to_owned(),
        }
    }

    /// The mapping's refusal line in words ("Map a column to Identity ▸ Maker", "Identity ▸ Part number is chosen for two
    /// columns"); `None` without a refusal. A failure of the last read is shown as it is. [`PartsMap::error`] keeps the
    /// importer's message for the describe text and the action's outcome.
    pub fn error_text(&self) -> Option<String> {
        let error = self.error.as_ref()?;
        if let MapKind::Report(target) = self.kind {
            let mut chosen = std::collections::BTreeSet::new();
            if let Some(m) = self
                .columns
                .iter()
                .filter_map(|c| c.member.as_deref())
                .find(|m| !chosen.insert(*m))
            {
                return Some(strings::format(
                    key!("parts-map-twice-text"),
                    &[&self.member_words(m, "")],
                ));
            }
            if let Some((m, _)) = target
                .members()
                .iter()
                .find(|(m, required)| *required && !chosen.contains(m))
            {
                return Some(strings::format(
                    key!("parts-map-missing"),
                    &[&self.member_words(m, "")],
                ));
            }
            return Some(error.clone());
        }
        let mut chosen = std::collections::BTreeSet::new();
        if let Some(c) = self
            .columns
            .iter()
            .find(|c| c.member.as_deref().is_some_and(|m| !chosen.insert(m)))
        {
            let member = c.member.as_deref().unwrap_or_default();
            return Some(strings::format(
                key!("parts-map-twice-text"),
                &[&Self::member_text(member, &c.header)],
            ));
        }
        let pairs = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref().map(|m| (m, c.header.as_str())));
        Some(match catalog::ColumnMap::new(None, pairs) {
            Err(catalog::MapError::Missing(m)) => {
                strings::format(key!("parts-map-missing"), &[&Self::member_text(m, "")])
            }
            Err(catalog::MapError::MixedDescription) => strings::text(key!("parts-map-mixed")).to_owned(),
            _ => error.clone(),
        })
    }

    /// The member a header names by its words or its member's name, without regard to case (a table exported from
    /// Parts, a supplier's "Description"): what a column without a saved map is pre-filled with.
    fn named_by(header: &str) -> Option<String> {
        let h = header.trim().to_lowercase();
        if h.is_empty() {
            return None;
        }
        // A column's own technical value or free property is mapped by choice only; a named technical value is a field.
        let named = |m: &String| {
            m != "like"
                && !m.starts_with("free_props.")
                && (!m.starts_with("tech_data.") || ew_api::part_fields::field(m).is_some())
        };
        Self::choices(header).into_iter().filter(named).find(|m| {
            m.to_lowercase() == h
                // A cable's cores by their own name ("Cores").
                || (Self::cable_word(m).is_some() && m.rsplit('.').next() == Some(h.as_str()))
                || Self::member_text(m, header).to_lowercase() == h
                || Self::plain_word(m).is_some_and(|w| w.to_lowercase() == h)
        })
    }

    /// A field's word without its group ("Maker", "Part number"), as a header names it.
    fn plain_word(member: &str) -> Option<String> {
        ew_api::part_fields::field(member)
            .map(|f| strings::render(&ew_api::part_fields::word(f.word)))
            .or_else(|| Self::cable_word(member).map(str::to_owned))
    }

    /// The word of a cable's cores column ([`ew_api::parts_set::CABLE_CORES`]) as the sheet's Cores form says it.
    fn cable_word(member: &str) -> Option<&'static str> {
        ew_api::parts_set::CABLE_CORES
            .contains(&member)
            .then(|| super::part_dialog::cable_label(member.trim_start_matches("cable.")))
    }
}

/// Why a row of the set makes no part, in words (the end of the sentence that names the row).
fn refusal_words(why: &SetRefusal) -> String {
    use ew_api::catalog::RowRefusal as R;
    match why {
        SetRefusal::Read(r) => match r {
            R::NoPartNumber => strings::text(key!("parts-set-no-number")).to_owned(),
            R::NoManufacturer => strings::text(key!("parts-set-no-maker")).to_owned(),
            R::ErrorCell { column, error } => strings::format(key!("parts-set-error-cell"), &[column, error]),
            R::Ambiguous { maker, keys } => {
                strings::format(key!("parts-set-maker-ambiguous"), &[maker, &keys.join(", ")])
            }
            R::MakerWithoutLetters(t) => strings::format(key!("parts-set-maker-letters"), &[t]),
            R::NumberWithoutLetters(t) => strings::format(key!("parts-set-number-letters"), &[t]),
            R::Repeats { line } => strings::format(key!("parts-set-repeats-other"), &[&line.to_string()]),
            R::NoNextVersion(_) => strings::text(key!("parts-set-no-version")).to_owned(),
            R::Value(_) | R::Host(_) => strings::text(key!("parts-set-unread")).to_owned(),
        },
        SetRefusal::Field { column, message } => strings::format(
            key!("parts-set-field"),
            &[
                column,
                &super::part_dialog::message_text(strings::Key::run_time(message.key), &message.args),
            ],
        ),
        SetRefusal::Like(number) => strings::format(key!("parts-set-like"), &[number]),
        // A part he can read that is not his: the way in, in the words of the sheet of several parts.
        SetRefusal::NotHis { number, lives } => match lives {
            ew_api::parts::Lives::NotMine { starter: true, .. } => {
                strings::format(key!("parts-set-starter"), &[number])
            }
            ew_api::parts::Lives::NotMine { library, .. } => {
                strings::format(key!("parts-set-library"), &[number, library])
            }
            _ => strings::text(key!("parts-set-here")).to_owned(),
        },
    }
}

/// A changed row's changes as the card of an update names them: each member's words, what it holds now and what it
/// will hold.
pub fn row_changes(row: &ew_api::parts_set::SetRow) -> Vec<(String, String, String)> {
    super::part_dialog::card_fields(&row.changes)
}

/// What a row of the set is, in a sentence of What was read: refused with why, the same, a repeat; `None` for a new
/// or a changed row.
pub fn row_sentence(row: &ew_api::parts_set::SetRow) -> Option<String> {
    let line = row.line.to_string();
    let base = row_status_sentence(row, &line);
    // A Like the row does not use, said rather than dropped.
    if !row.like_unused {
        return base;
    }
    let like = strings::format(key!("parts-set-like-unused"), &[&line, &row.number]);
    Some(base.map_or_else(|| like.clone(), |b| format!("{b} {like}")))
}

/// The sentence of a row by what it is: refused with why, the same, a repeat, changed with the fields it changes.
fn row_status_sentence(row: &ew_api::parts_set::SetRow, line: &str) -> Option<String> {
    let line = line.to_owned();
    match &row.status {
        SetStatus::Refused(why) => Some(strings::format(
            key!("parts-set-refused-line"),
            &[&line, &refusal_words(why)],
        )),
        SetStatus::Same => Some(strings::format(
            key!("parts-set-same-line"),
            &[&line, &row.number],
        )),
        SetStatus::Repeats(of) => Some(strings::format(
            key!("parts-set-repeats-line"),
            &[&line, &row.number, &of.to_string()],
        )),
        SetStatus::Changed => {
            let words: Vec<String> = row_changes(row).into_iter().map(|(w, _, _)| w).collect();
            Some(strings::format(
                key!("parts-set-changed-line"),
                &[&line, &row.number, &words.join(", ")],
            ))
        }
        SetStatus::New => None,
    }
}

/// Why a row of the set makes no part, in words, without the row's number (the list's second line); `None` for a row
/// that makes one.
pub fn row_reason(row: &ew_api::parts_set::SetRow) -> Option<String> {
    match &row.status {
        SetStatus::Refused(why) => Some(refusal_words(why)),
        _ => None,
    }
}

/// The chip of a row of the set: new, changed, the same, refused.
pub fn row_chip(status: &SetStatus) -> &'static str {
    strings::text(match status {
        SetStatus::New => key!("parts-set-chip-new"),
        SetStatus::Changed => key!("parts-set-chip-changed"),
        SetStatus::Same | SetStatus::Repeats(_) => key!("parts-set-chip-same"),
        SetStatus::Refused(_) => key!("parts-set-chip-refused"),
    })
}

impl ShellModel {
    /// The EPLAN report's column mapping card while it is open.
    pub fn parts_map(&self) -> Option<&PartsMap> {
        self.parts_map.as_ref()
    }

    /// The set an import brought into Parts, while it shows.
    pub fn import_set(&self) -> Option<&ImportSet> {
        self.parts.as_deref()?.set.as_deref()
    }

    /// `workshop.add_generic`: Add the generic stock (doc 07 §2.5): the starter's generic ferrules and markers the
    /// project's workshop stock lacks join it after its own, one step; refused in words when it holds them all.
    pub(crate) fn act_workshop_add_generic(&mut self) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let Some(cmd) = project.session.add_generic_stock_command() else {
            return Outcome::refused(key!("reason-workshop-generic-held"));
        };
        self.run_commit(&cmd)
    }

    /// `parts.import {path?, index?, key?, discard?, terms?, update?}` (Import… of Parts; module documentation): the
    /// file at `path` goes on by what it is and where Parts stands (the Symbols shelf takes a symbol file); without
    /// `path` the window's file dialog asks. `index` takes another drawing of the symbol file in hand; `update` previews
    /// the update of the project's copies of an EPLAN set's parts.
    pub(crate) fn act_parts_import(&mut self, p: &Map<String, Value>) -> Outcome {
        if flag(p, "update") {
            return self.set_update();
        }
        if p.contains_key("index") {
            return self.act_symbol("candidate", p);
        }
        // A symbol file: one only a symbol is read from, or one of the files both read (an EPLAN file, a DXF drawing)
        // on the drawing or the Symbols shelf. Its file dialog is the symbol files'.
        let extension = or_return!(string_param(p, "path")).map(|path| {
            std::path::Path::new(path)
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default()
        });
        let drawing =
            self.symbol_editor().is_some() || self.parts.as_deref().is_some_and(|place| place.shelf_symbols);
        let parts_too = |e: &str| matches!(e, "csv" | "xlsx" | "dxf") || super::EplanKind::of(e).is_some();
        let symbol = match extension.as_deref() {
            Some(e) if super::symbol_editor::FILE_EXTENSIONS.contains(&e) && !parts_too(e) => true,
            Some("csv" | "xlsx") => false,
            _ => drawing,
        };
        if symbol {
            let mut q = p.clone();
            if let Some(path) = q.remove("path") {
                q.insert("file".to_owned(), path);
            }
            return self.act_symbol("open", &q);
        }
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("parts.import", Prompt::OpenPartsTable, "path", p),
        };
        let file = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "csv" | "xlsx" => self.import_table(path, file),
            "dxf" => self.act_parts_outline(p),
            e if super::EplanKind::of(e).is_some_and(|k| k != super::EplanKind::Report) => {
                self.import_eplan_parts(path, file, flag(p, "terms"))
            }
            _ => Outcome::refused(key!("reason-import-kind")),
        }
    }

    /// The place shown for an import's set: a sheet with changes not saved is asked about first (a script refused in
    /// words). `None` where the import goes on.
    fn place_for_set(&mut self) -> Option<Outcome> {
        if self
            .parts
            .as_deref()
            .and_then(|p| p.sheet.as_ref())
            .is_some_and(|o| o.changed)
            || self.many_sheet().is_some_and(|m| m.changed)
        {
            return Some(Outcome::refused(key!("reason-parts-unsaved")));
        }
        self.enter_place();
        None
    }

    /// A table: its columns and the set read through the map pre-filled for it.
    fn import_table(&mut self, path: PathBuf, file: String) -> Outcome {
        let limits = catalog::table_limits();
        let read = catalog::read_input(&path, &limits).map_err(|e| e.to_string());
        let (input, columns) = match read.and_then(|input| {
            catalog::table_columns(&input, None, &limits)
                .map(|c| (input, c))
                .map_err(|e| e.to_string())
        }) {
            Ok(r) => r,
            Err(why) => return failed(key!("error-parts-import"), &[&why]),
        };
        if let Some(out) = self.place_for_set() {
            return out;
        }
        let mut map = PartsMap {
            kind: MapKind::Parts,
            header_row: 0,
            source: catalog::source_key_for(&file),
            file,
            path,
            saved: false,
            columns: columns
                .into_iter()
                .map(|c| MapColumn {
                    header: c.header,
                    sample: c.sample,
                    member: None,
                })
                .collect(),
            error: None,
        };
        self.prefill(&mut map);
        self.read_set(input, map);
        Outcome::Done
    }

    /// The set of the table `input` read through `map`, shown in the place (an empty set under the mapping's refusal
    /// while the map is not usable).
    fn read_set(&mut self, input: catalog::ImportInput, mut map: PartsMap) {
        let set = match (map.column_map(), self.parts_session()) {
            (Ok(columns), Some(session)) => match session.table_set(&input, &columns) {
                Ok(set) => {
                    map.error = None;
                    Some(set)
                }
                Err(e) => {
                    map.error = Some(e.to_string());
                    None
                }
            },
            (Err(e), _) => {
                map.error = Some(e);
                None
            }
            (Ok(_), None) => None,
        };
        self.show_set(set, Some(map), Some(input), None);
    }

    /// Shows the set in the place: the list on This import, the first row shown as it will be.
    fn show_set(
        &mut self,
        set: Option<PartsSet>,
        map: Option<PartsMap>,
        input: Option<catalog::ImportInput>,
        project: Option<(usize, usize)>,
    ) {
        let Some(place) = self.parts.as_deref_mut() else {
            return;
        };
        let keep_row = place.set.as_deref().and_then(|s| s.row);
        let rows = set.as_ref().map_or(0, |s| s.rows.len());
        let file = set
            .as_ref()
            .map(|s| s.file.clone())
            .or_else(|| map.as_ref().map(|m| m.file.clone()))
            .unwrap_or_default();
        place.sheet = None;
        place.many = None;
        place.asking = None;
        place.shelf_symbols = false;
        place.scope = ew_api::parts::PartScope::Import;
        place.set = Some(Box::new(ImportSet {
            file,
            row: keep_row.filter(|r| *r < rows).or((rows > 0).then_some(0)),
            set,
            map,
            input,
            project,
            held: None,
            texts: Vec::new(),
            ready: Vec::new(),
        }));
        self.parts_refresh();
        self.set_row_shown();
    }

    /// The shown row of the set as its sheet reads it, and its Ready for.
    pub(super) fn set_row_shown(&mut self) {
        let (Some(session), Some(place)) = self.parts_session_mut() else {
            return;
        };
        let Some(shown) = place.set.as_deref_mut() else {
            return;
        };
        let part = shown.shown().and_then(|r| r.part.clone());
        (shown.texts, shown.ready) = match part {
            Some(p) => (session.set_row_texts(&p), ew_api::part_fields::ready_for(&p)),
            None => (Vec::new(), Vec::new()),
        };
    }

    /// An EPLAN parts file: the terms notice first for an EDZ, once per session; then the file read as a job against
    /// My parts, its set shown at the job's end.
    fn import_eplan_parts(&mut self, path: PathBuf, file: String, terms: bool) -> Outcome {
        // The notice's Continue: the notice closes as the file is read.
        if terms && matches!(self.dialog, Some(Dialog::ImportEplan { .. })) {
            self.dialog = None;
        }
        self.enter_place();
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let at = ew_api::parts_set::SetTarget::of(session.libraries());
        // The open project's copies of the file's parts: the file read for the project, as its update reads it.
        let project = self.project.as_ref().map(|p| p.session.eplan_parts_read(None));
        let terms_seen = self.import.edz_terms;
        let config_dir = self.config.config_dir.clone();
        let name = file.clone();
        self.start_read(&file, move |_| {
            let Ok(input) = catalog::read_input(&path, &catalog::table_limits()) else {
                return Ok(
                    Box::new(move |_: &mut ShellModel| failed(key!("error-import-read"), &[&name])) as Finish,
                );
            };
            // The notice belongs to an EDZ whatever the file is called.
            let edz = eplan::is_edz(input.bytes());
            if edz && !terms_seen && !terms {
                let shown = path.to_string_lossy().into_owned();
                return Ok(Box::new(move |s: &mut ShellModel| {
                    s.open_dialog(Dialog::ImportEplan {
                        path: shown,
                        name,
                        step: EplanStep::Terms,
                    })
                }) as Finish);
            }
            let read = eplan::fndef_map(config_dir.as_deref()).and_then(|fndefs| {
                let set = at.read_eplan(&input, &fndefs).map_err(|e| e.to_string())?;
                let held = project.and_then(|p| p.run(&input, &fndefs).ok());
                Ok(EplanRead { set, held, fndefs })
            });
            Ok(
                Box::new(move |s: &mut ShellModel| s.eplan_set_read(&name, input, edz && terms, read))
                    as Finish,
            )
        })
    }

    /// The end of an EPLAN parts file's read: its set in the place, with how many of its parts the open project holds a
    /// copy of and how many of those would change.
    fn eplan_set_read(
        &mut self,
        name: &str,
        input: catalog::ImportInput,
        terms: bool,
        read: Result<EplanRead, String>,
    ) -> Outcome {
        if terms {
            self.import.edz_terms = true;
        }
        let EplanRead { set, held, fndefs } = match read {
            Ok(r) => r,
            Err(why) => return self.import_failed(name, &why),
        };
        if let Some(out) = self.place_for_set() {
            return out;
        }
        let Some(session) = self.parts_session() else {
            return Outcome::Done;
        };
        let set = session.eplan_set(&input, &fndefs, set);
        let project = self.project.as_ref().zip(held.as_ref()).and_then(|(p, held)| {
            let updates = p.session.update_parts_from_import(held).ok()?;
            let changed = updates.plan.as_ref().map_or(0, |plan| plan.copies.len());
            (!updates.matched.is_empty()).then_some((updates.matched.len(), changed))
        });
        self.show_set(Some(set), None, Some(input), project);
        if let Some(shown) = self.parts.as_deref_mut().and_then(|p| p.set.as_deref_mut()) {
            shown.held = held;
        }
        Outcome::Done
    }

    /// `parts.import {update}`: the previewed update of the open project's copies of the EPLAN set's parts (FR-PRT-014),
    /// on the card beside the place; Apply commits it in one step.
    fn set_update(&mut self) -> Outcome {
        let Some(held) = self.import_set().and_then(|s| s.held.as_ref()) else {
            return Outcome::refused(key!("reason-no-import-set"));
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let plan = match project.session.update_parts_from_import(held) {
            Ok(u) => u.plan,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let Some(plan) = plan else {
            return Outcome::refused(key!("reason-eplan-no-change"));
        };
        self.preview_host_card(
            &plan.command.name,
            plan.command.params,
            key!("reason-library-update"),
        )
    }

    /// Save of the set (Save n to My parts): its new and changed parts written into My parts, read again as My parts is
    /// now; a table's map saved for its source. The list then shows My parts.
    pub(crate) fn save_set(&mut self) -> Outcome {
        let Some(shown) = self.import_set() else {
            return Outcome::refused(key!("reason-no-import-set"));
        };
        let Some(set) = shown.set.clone() else {
            return Outcome::refused(key!("reason-no-import-set"));
        };
        let map = shown.map.clone();
        let (Some(session), _) = self.parts_session_mut() else {
            return Outcome::refused(key!("reason-no-parts-place"));
        };
        let (new, changed) = match session.save_set(&set) {
            Ok(c) => c,
            // My parts changed since the table was read: nothing is written, the rows are judged anew and shown.
            Err(ew_api::parts_set::SetSaveError::Changed(fresh)) => {
                if let Some(shown) = self.parts.as_deref_mut().and_then(|p| p.set.as_deref_mut()) {
                    shown.set = Some(*fresh);
                }
                self.parts_refresh();
                self.set_row_shown();
                return Outcome::refused(key!("reason-set-read-again"));
            }
            Err(ew_api::parts_set::SetSaveError::Catalog(catalog::CatalogError::Library(
                ew_api::library::LibraryError::NotFound,
            ))) => {
                return failed(key!("error-part-no-my-parts"), &[]);
            }
            Err(ew_api::parts_set::SetSaveError::Catalog(e)) => {
                return failed(key!("error-parts-import"), &[&e.to_string()]);
            }
        };
        // The map is saved with the set it read, not before.
        if let (Some(map), Some(dir)) = (&map, self.config.config_dir.as_deref())
            && let Ok(columns) = map.column_map()
            && let Err(e) = catalog::save_column_map(dir, &map.source, &columns)
        {
            return Outcome::Failed {
                message: e.to_string(),
            };
        }
        self.libraries_changed();
        if let Some(place) = self.parts.as_deref_mut() {
            place.set = None;
            place.scope = ew_api::parts::PartScope::Mine;
        }
        self.parts_refresh();
        self.notify(
            strings::format(
                key!("toast-parts-set-saved"),
                &[&set.file, &new.to_string(), &changed.to_string()],
            ),
            false,
        );
        Outcome::Done
    }

    /// Discard of the set: it goes, nothing of it saved; the list shows All again.
    pub(crate) fn discard_set(&mut self) -> Outcome {
        if let Some(place) = self.parts.as_deref_mut() {
            place.set = None;
            if place.scope == ew_api::parts::PartScope::Import {
                place.scope = ew_api::parts::PartScope::All;
            }
        }
        self.parts_refresh();
        Outcome::Done
    }

    /// Fills the members from the map saved for the source key (`saved`), else from the headers that name a field.
    fn prefill(&self, map: &mut PartsMap) {
        if matches!(map.kind, MapKind::Report(_)) {
            return self.prefill_report(map);
        }
        let saved = self
            .config
            .config_dir
            .as_deref()
            .and_then(|dir| catalog::load_column_map(dir, &map.source).ok());
        map.saved = saved.is_some();
        for c in &mut map.columns {
            c.member = match &saved {
                Some(m) => m
                    .columns()
                    .find(|(_, h)| *h == c.header)
                    .map(|(member, _)| member.name()),
                None => PartsMap::named_by(&c.header),
            };
        }
        // A member chosen twice by the headers is kept for the first column only.
        let mut chosen = std::collections::BTreeSet::new();
        for c in &mut map.columns {
            if let Some(m) = &c.member
                && !chosen.insert(m.clone())
                && !map.saved
            {
                c.member = None;
            }
        }
        map.error = map.column_map().err();
    }

    /// Whether the table `input` of `path` reads as a parts table without asking: a map is saved for its source key,
    /// or its headers name the maker and the part number (File ▸ Import… then goes on to Parts).
    pub(super) fn reads_as_parts_table(&self, path: &std::path::Path, input: &catalog::ImportInput) -> bool {
        let Ok(columns) = catalog::table_columns(input, None, &catalog::table_limits()) else {
            return false;
        };
        let file = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let saved = self
            .config
            .config_dir
            .as_deref()
            .is_some_and(|dir| catalog::load_column_map(dir, &catalog::source_key_for(&file)).is_ok());
        let named: Vec<String> = columns
            .iter()
            .filter_map(|c| PartsMap::named_by(&c.header))
            .collect();
        saved
            || ["manufacturer", "part_number"]
                .iter()
                .all(|m| named.iter().any(|n| n == m))
    }

    /// Fills a report's members from the mapping saved for the source key (and takes its kind).
    fn prefill_report(&self, map: &mut PartsMap) {
        let saved = self
            .config
            .config_dir
            .as_deref()
            .and_then(|dir| eplan::load_report_map(dir, &map.source));
        map.saved = saved.is_some();
        if let Some(m) = &saved {
            map.kind = MapKind::Report(m.target);
        }
        for c in &mut map.columns {
            c.member = saved.as_ref().and_then(|m| {
                m.columns
                    .iter()
                    .find(|(_, h)| h.trim() == c.header)
                    .map(|(member, _)| member.clone())
            });
        }
        map.error = map.report_map().err();
    }

    /// The mapping step of the report spreadsheet `path` (read as `input`): its columns as detected, the members of
    /// the mapping saved for the file's source key (else none), a terminal diagram unless that mapping says another
    /// (FR-IMP-029).
    pub(super) fn open_report_map(&mut self, path: PathBuf, input: &catalog::ImportInput) -> Outcome {
        let file = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let (columns, header_row) = match eplan::report_columns(input) {
            Ok(c) => c,
            Err(e) => return self.import_failed(&file, &e.to_string()),
        };
        let mut map = PartsMap {
            kind: MapKind::Report(ReportTarget::Terminals),
            header_row,
            source: catalog::source_key_for(&file),
            file,
            path,
            saved: false,
            columns: columns
                .into_iter()
                .map(|c| MapColumn {
                    header: c.header,
                    sample: c.sample,
                    member: None,
                })
                .collect(),
            error: None,
        };
        self.prefill_report(&mut map);
        self.dialog = None;
        self.parts_map = Some(map);
        Outcome::Done
    }

    /// Import on a report's mapping: the mapping is saved under the source key (the next file of the report scheme
    /// finds it), then the report becomes a new project in `dir` (asked for when absent).
    fn report_map_import(&mut self, map: PartsMap, p: &Map<String, Value>) -> Outcome {
        let report = match map.report_map() {
            Ok(m) => m,
            Err(message) => return Outcome::Failed { message },
        };
        if let Some(dir) = self.config.config_dir.as_deref()
            && let Err(e) = eplan::save_report_map(dir, &map.source, &report)
        {
            return self.map_failed(e.to_string());
        }
        let input = match catalog::read_input(&map.path, &catalog::table_limits()) {
            Ok(i) => i,
            Err(e) => return self.map_failed(e.to_string()),
        };
        let outcome = self.run_report_import(p, &map.file, &input, report, &map.source);
        if outcome == Outcome::Done && self.dialog.is_none() && self.prompt.is_none() {
            self.parts_map = None;
        }
        outcome
    }

    /// `parts.map {source?, column?, member?, target?}`: sets the source key (a map saved for it fills the members)
    /// and the member the column with header `column` imports as (`member` empty or absent: not imported). On the set
    /// of a table in Parts the set is read again at once; else the EPLAN report's mapping card (`target` its kind).
    pub(crate) fn act_parts_map(&mut self, p: &Map<String, Value>) -> Outcome {
        let (source, column, member, target) = match (
            string_param(p, "source"),
            string_param(p, "column"),
            string_param(p, "member"),
            string_param(p, "target"),
        ) {
            (Ok(s), Ok(c), Ok(m), Ok(t)) => (
                s.map(str::to_owned),
                c.map(str::to_owned),
                m.map(str::to_owned),
                t.map(str::to_owned),
            ),
            (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => return e,
        };
        let in_place = self.import_set().is_some_and(|s| s.map.is_some());
        let taken = if in_place {
            self.parts
                .as_deref_mut()
                .and_then(|p| p.set.as_deref_mut())
                .and_then(|s| s.map.take())
        } else {
            self.parts_map.take()
        };
        let Some(mut map) = taken else {
            return Outcome::refused(key!("reason-no-parts-map"));
        };
        let outcome = (|| {
            // A report's kind: the members of the other kinds are dropped.
            if let Some(t) = target {
                let MapKind::Report(_) = map.kind else {
                    return failed(key!("error-bad-param"), &["target"]);
                };
                let kind = match t.as_str() {
                    "devices" => ReportTarget::Devices,
                    "terminals" => ReportTarget::Terminals,
                    "connections" => ReportTarget::Connections,
                    _ => return failed(key!("error-bad-param"), &["target"]),
                };
                map.kind = MapKind::Report(kind);
                for c in &mut map.columns {
                    if c.member
                        .as_deref()
                        .is_some_and(|m| !kind.members().iter().any(|(k, _)| k == &m))
                    {
                        c.member = None;
                    }
                }
            }
            if let Some(source) = source {
                let source = source.trim().to_owned();
                if let Err(message) = catalog::check_source_key(&source) {
                    return Outcome::Failed { message };
                }
                if source != map.source {
                    map.source = source;
                    let had = map.columns.iter().map(|c| c.member.clone()).collect::<Vec<_>>();
                    self.prefill(&mut map);
                    // A key without a saved map keeps the members chosen so far.
                    if !map.saved {
                        for (c, m) in map.columns.iter_mut().zip(had) {
                            c.member = m;
                        }
                    }
                }
            }
            if let Some(header) = column {
                let Some(i) = map.columns.iter().position(|c| c.header == header) else {
                    return failed(key!("error-bad-param"), &["column"]);
                };
                let member = member.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
                if let Some(m) = &member
                    && !map.member_choices(&header).contains(m)
                {
                    return failed(key!("error-bad-param"), &["member"]);
                }
                map.columns[i].member = member;
            }
            Outcome::Done
        })();
        map.error = match map.kind {
            MapKind::Parts => map.column_map().err(),
            MapKind::Report(_) => map.report_map().err(),
        };
        if in_place {
            let input = self.import_set().and_then(|s| s.input.clone());
            match input {
                Some(input) => self.read_set(input, map),
                None => {
                    if let Some(s) = self.parts.as_deref_mut().and_then(|p| p.set.as_deref_mut()) {
                        s.map = Some(map);
                    }
                }
            }
        } else {
            self.parts_map = Some(map);
        }
        outcome
    }

    /// `parts.map_import`: Import on the EPLAN report's mapping card (the report becomes a new project, `dir` asked for
    /// when absent); refused while the map is not usable (its message under the table).
    pub(crate) fn act_parts_map_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(map) = self.parts_map.clone() else {
            return Outcome::refused(key!("reason-no-parts-map"));
        };
        self.report_map_import(map, p)
    }

    /// A failed import of the mapping step: the message under the table, the card stays open.
    fn map_failed(&mut self, message: String) -> Outcome {
        if let Some(m) = self.parts_map.as_mut() {
            m.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }
}

/// An EPLAN parts file's read, for its end ([`ShellModel::eplan_set_read`]): the set against My parts, the file read
/// for the open project's copies, and the function definitions it was read with.
struct EplanRead {
    set: catalog::PartsImport,
    held: Option<catalog::PartsImport>,
    fndefs: eplan::FndefMap,
}

impl ShellModel {
    /// The card of part copy `part` (held or of an installed library, F22); `None` without a project or such a part.
    pub fn part_card(&self, part: ew_api::output::Uuid) -> Option<crate::part_card::PartCardView> {
        let card = self.project.as_ref()?.session.part_card(part)?;
        Some(crate::part_card::PartCardView::of(card))
    }

    /// The card of the first part of device `device`; `None` for a device without a part.
    pub fn device_part_card(&self, device: ew_api::output::Uuid) -> Option<crate::part_card::PartCardView> {
        self.part_card(self.device_part(device)?)
    }

    /// The part copy of device `device`'s first part use; `None` without a project or a part.
    pub fn device_part(&self, device: ew_api::output::Uuid) -> Option<ew_api::output::Uuid> {
        self.project.as_ref()?.session.device_part(device)
    }
}
