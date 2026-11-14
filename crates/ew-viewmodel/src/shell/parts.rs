// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File, Import, Parts table…: the headless action `parts.import`.
//!
//! The action asks for a `parts.csv` (the window's file dialog, or the `path` parameter), reads it with the tables
//! beside it through `ew_api::import::read_parts_tables`, and previews the `parts.import` command through the host.
//! The command's cost is Bulk, so the inline preview card always shows: Apply on a fresh card commits one transaction
//! (one undo step) and leaves the banner "Imported 29 parts from parts.csv (0 skipped)."; a stale card previews the
//! same copies again and shows a fresh card that needs a second Apply. A refused table shows its first problem, with
//! its pointer, in the banner and opens no card. The registry's `writable` check has run before the action body
//! (`ShellModel::dispatch`).
//!
//! **The column mapping step (D11).** `parts.import {path}` first opens the mapping card on any
//! table (CSV or XLSX): the source key (pre-filled from the file name), and per column its header, a sample value and
//! the member it imports as, pre-filled from the map saved for the source key (`<config dir>/imports/<key>.toml`),
//! else from the golden dialect's headers when the file has exactly those. `parts.map` changes the key or a column's
//! member; `parts.map_import` saves the map and reads the table through `ew_api::catalog`, whose new parts go to the
//! import card above. A golden-dialect table imported with its pre-filled map unchanged is read by the golden reader
//! with its sibling tables (part functions, cores), exactly as before the step. Escape (`ui.dismiss`) closes the card.

use std::path::PathBuf;

use ew_api::catalog;
use ew_api::eplan::{self, ReportMap, ReportTarget};
use ew_api::import::{PartsImport, parts_import_counts, read_parts_tables};
use ew_api::{AnyCommand, Submit};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Outcome, Prompt, ShellModel, failed, string_param};
use crate::key;
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

/// The name of the command the action dispatches.
const COMMAND: &str = "parts.import";

/// The columns of the golden dialect's `parts.csv` (`ew-import-tables`) and the member each fills in a column map:
/// the required `key`, `manufacturer` and `part_number`, then the optional ones; `key`, `test_data`, `tech_data` (a
/// `name=value;…` list), the mechanical columns (`width` … `rail_profile`, read by the golden reader) and
/// `solenoids` (a valve part's fluid section) fill no member of the general importer.
const GOLDEN_COLUMNS: [(&str, Option<&str>); 18] = [
    ("key", None),
    ("manufacturer", Some("manufacturer")),
    ("part_number", Some("part_number")),
    ("kind", Some("kind")),
    ("type_designation", Some("type_designation")),
    ("description", Some("description")),
    ("unit", Some("unit")),
    ("test_data", None),
    ("tech_data", None),
    ("width", None),
    ("height", None),
    ("depth", None),
    ("mount_kind", None),
    ("enclosure", None),
    ("enclosure_plate", None),
    ("plate", None),
    ("rail_profile", None),
    ("solenoids", None),
];

/// The golden pre-fill of a table whose headers are exactly golden-dialect columns, the three required ones among
/// them, each once: one member per column. `None` for any other table.
fn golden_members(columns: &[catalog::TableColumn]) -> Option<Vec<Option<String>>> {
    let golden = |h: &str| GOLDEN_COLUMNS.iter().find(|(c, _)| *c == h);
    let mut seen = std::collections::BTreeSet::new();
    let all = columns
        .iter()
        .all(|c| golden(&c.header).is_some() && seen.insert(c.header.as_str()));
    let required = ["key", "manufacturer", "part_number"]
        .iter()
        .all(|r| seen.contains(r));
    (all && required).then(|| {
        columns
            .iter()
            .map(|c| golden(&c.header).and_then(|(_, m)| m.map(str::to_owned)))
            .collect()
    })
}

/// One column of the mapping card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapColumn {
    /// The header.
    pub header: String,
    /// The first value under it.
    pub sample: String,
    /// The member it imports as (`part_number`, `tech_data.i_n`); `None`: not imported.
    pub member: Option<String>,
}

/// What the column mapping step maps for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapKind {
    /// A supplier's parts table (the members of the parts import).
    Parts,
    /// An EPLAN report spreadsheet of the given kind (FR-IMP-029, FT-1742): the members of that report.
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
        _ => return None,
    })
}

/// The column mapping step of File ▸ Import… of a parts table or of an EPLAN report, while it is open.
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
    golden: Option<Vec<Option<String>>>,
    /// Why Import is refused: the importer's message for the map (`the column map needs a column for
    /// `part_number``), or the last import's failure.
    pub error: Option<String>,
}

impl PartsMap {
    /// The member choices of the column `header`: the members without a key in declaration order, then the
    /// column's own technical-data and free-property members (`tech_data.<header as identifier>`), when the header
    /// gives an identifier the importer takes.
    pub fn choices(header: &str) -> Vec<String> {
        let mut out: Vec<String> = catalog::Member::FIXED.iter().map(catalog::Member::name).collect();
        let ident: String = header
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let ident = ident.trim_matches('_').to_owned();
        for head in ["tech_data", "free_props"] {
            let name = format!("{head}.{ident}");
            if !ident.is_empty() && catalog::Member::parse(&name).is_ok() {
                out.push(name);
            }
        }
        out
    }

    /// The member choices of the column `header` for this step: a parts table's ([`PartsMap::choices`]), or the members
    /// of the report's kind.
    pub fn member_choices(&self, header: &str) -> Vec<String> {
        match self.kind {
            MapKind::Parts => Self::choices(header),
            MapKind::Report(target) => target.members().iter().map(|(m, _)| (*m).to_owned()).collect(),
        }
    }

    /// A member in words for this step ([`PartsMap::member_text`], or the report member's words).
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

    /// A member in words, as the mapping card shows it (doc 16a P2): "Manufacturer",
    /// "Part number", "Description (de)", "Technical data: Nennstrom", "Free property: Farbe", with `header` the
    /// column's header. A name that is no member is shown as it is. Describe text and scripts keep the member names.
    pub fn member_text(member: &str, header: &str) -> String {
        match catalog::Member::parse(member) {
            Ok(catalog::Member::Description(Some(lang))) => {
                strings::format(key!("parts-member-description-lang"), &[&lang])
            }
            Ok(catalog::Member::TechData(_)) => strings::format(key!("parts-member-tech-data"), &[header]),
            Ok(catalog::Member::FreeProp(_)) => strings::format(key!("parts-member-free-prop"), &[header]),
            Ok(m) => strings::text(match m {
                catalog::Member::Manufacturer => key!("parts-member-manufacturer"),
                catalog::Member::PartNumber => key!("parts-member-part-number"),
                catalog::Member::TypeDesignation => key!("parts-member-type-designation"),
                catalog::Member::OrderNumber => key!("parts-member-order-number"),
                catalog::Member::Gtin => key!("parts-member-gtin"),
                catalog::Member::Kind => key!("parts-member-kind"),
                catalog::Member::Unit => key!("parts-member-unit"),
                _ => key!("parts-member-description"),
            })
            .to_owned(),
            Err(_) => member.to_owned(),
        }
    }

    /// The card's refusal line in words ("Map a column to Manufacturer", "Part number is chosen for two columns");
    /// `None` without a refusal. A failure of the last import is shown as it is. [`PartsMap::error`] keeps the
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
}

impl ShellModel {
    /// Whether the card on show is an import card (a window of its own, which needs neither the grid nor the find
    /// pane).
    pub fn import_card_open(&self) -> bool {
        self.has_project()
            && self
                .props
                .card()
                .is_some_and(|c| matches!(c.origin, CardOrigin::Import { .. }))
    }

    /// The column mapping step while it is open.
    pub fn parts_map(&self) -> Option<&PartsMap> {
        self.parts_map.as_ref()
    }

    /// The banner and the failure of a table that cannot be read.
    fn parts_refused(&mut self, why: &str) -> Outcome {
        let message = strings::format(key!("error-parts-import"), &[why]);
        self.banner = Some(message.clone());
        Outcome::Failed { message }
    }

    /// `parts.import {path}`: reads the table's header row and opens the column mapping step on it,
    /// pre-filled from the map saved for the file's source key, else from the golden dialect's headers when the file
    /// has exactly those. A table that cannot be read shows its problem in the banner and opens nothing.
    pub(crate) fn act_parts_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => return self.need_path("parts.import", Prompt::OpenPartsTable, "path", p),
        };
        if self.project.is_none() {
            return Outcome::Done;
        }
        let limits = catalog::table_limits();
        let columns = match catalog::read_input(&path, &limits)
            .map_err(|e| e.to_string())
            .and_then(|input| catalog::table_columns(&input, None, &limits).map_err(|e| e.to_string()))
        {
            Ok(c) => c,
            Err(why) => return self.parts_refused(&why),
        };
        let file = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let golden = golden_members(&columns);
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
            golden,
            error: None,
        };
        self.prefill(&mut map);
        self.parts_map = Some(map);
        Outcome::Done
    }

    /// Fills the members from the map saved for the source key (`saved`), else from the golden dialect.
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
        for (i, c) in map.columns.iter_mut().enumerate() {
            c.member = match &saved {
                Some(m) => m
                    .columns()
                    .find(|(_, h)| *h == c.header)
                    .map(|(member, _)| member.name()),
                None => map.golden.as_ref().and_then(|g| g[i].clone()),
            };
        }
        map.error = map.column_map().err();
    }

    /// Whether the table `input` of `path` reads as a parts table without asking: a map is saved for its source key,
    /// or its headers are exactly those of the golden dialect (File ▸ Import… then goes on to the parts import).
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
        saved || golden_members(&columns).is_some()
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
    /// (FR-IMP-029, FT-1742).
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
            golden: None,
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

    /// `parts.map {source?, column?, member?}`: sets the source key (a map saved for it fills the members)
    /// and the member the column with header `column` imports as (`member` empty or absent: not imported).
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
        let Some(mut map) = self.parts_map.take() else {
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
                if let Some(m) = &member {
                    let refused = match map.kind {
                        MapKind::Parts => catalog::Member::parse(m).err().map(|e| e.to_string()),
                        MapKind::Report(_) => (!map.member_choices("").contains(m))
                            .then(|| strings::format(key!("error-bad-param"), &["member"])),
                    };
                    if let Some(message) = refused {
                        return Outcome::Failed { message };
                    }
                }
                map.columns[i].member = member;
            }
            Outcome::Done
        })();
        map.error = match map.kind {
            MapKind::Parts => map.column_map().err(),
            MapKind::Report(_) => map.report_map().err(),
        };
        self.parts_map = Some(map);
        outcome
    }

    /// `parts.map_import`: saves the map for the source key and reads the table through it
    /// (`ew_api::catalog`): the new parts go to the import card (`parts.import`, as before), the counts of the rows read
    /// to the toast. A golden-dialect table whose pre-filled map is unchanged is read by the golden reader with its
    /// sibling tables, as before the mapping step. Refused while the importer refuses the map (its message under the
    /// table).
    pub(crate) fn act_parts_map_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(map) = self.parts_map.clone() else {
            return Outcome::refused(key!("reason-no-parts-map"));
        };
        if matches!(map.kind, MapKind::Report(_)) {
            return self.report_map_import(map, p);
        }
        let columns = match map.column_map() {
            Ok(c) => c,
            Err(message) => return Outcome::Failed { message },
        };
        if let Some(dir) = self.config.config_dir.as_deref()
            && let Err(e) = catalog::save_column_map(dir, &map.source, &columns)
        {
            return self.map_failed(e.to_string());
        }
        let golden = map
            .golden
            .as_ref()
            .is_some_and(|g| map.columns.iter().zip(g).all(|(c, m)| c.member == *m));
        let Some(project) = self.project.as_ref() else {
            return Outcome::Done;
        };
        if golden {
            let read = match read_parts_tables(&map.path, project.session.query().project().id.as_uuid()) {
                Ok(read) => read,
                Err(e) => return self.map_failed(e.to_string()),
            };
            self.parts_map = None;
            let params = read.params();
            let PartsImport { source, copies, .. } = read;
            let outcome = self.preview_import(params, source.clone(), copies.len());
            // The read's outcome is one toast, as on the mapped path; the card shows the list only.
            let origin = self.props.card().map(|c| c.origin.clone());
            if let Some(CardOrigin::Import { created, skipped, .. }) = origin {
                let text = strings::format(
                    key!("toast-parts-read"),
                    &[&source, &created.to_string(), &skipped.to_string(), "0"],
                );
                self.notify(text, false);
            }
            return outcome;
        }
        let limits = catalog::table_limits();
        let input = match catalog::read_input(&map.path, &limits) {
            Ok(i) => i,
            Err(e) => return self.map_failed(e.to_string()),
        };
        let master = catalog::manufacturer_master(project.session.libraries());
        let (import, cmd) = match project.session.catalog_parts(&input, &columns, &master, None) {
            Ok(r) => r,
            Err(e) => return self.map_failed(e.to_string()),
        };
        self.parts_map = None;
        let c = import.counts();
        let held = (c.updated + c.unchanged + c.duplicate).to_string();
        let rejected = c.rejected.to_string();
        // Every outcome of the read is one toast line, never a banner over the tab title.
        let Some(cmd) = cmd else {
            let message = strings::format(key!("banner-parts-none"), &[&map.file, &held, &rejected]);
            self.notify(message, false);
            return Outcome::Done;
        };
        self.notify(
            strings::format(
                key!("toast-parts-read"),
                &[&map.file, &c.new.to_string(), &held, &rejected],
            ),
            false,
        );
        let copies = c.new;
        self.preview_import(cmd.params, map.file, copies)
    }

    /// A failed import of the mapping step: the message under the table, the card stays open.
    fn map_failed(&mut self, message: String) -> Outcome {
        if let Some(m) = self.parts_map.as_mut() {
            m.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    /// Previews `parts.import` with `params` (a Bulk command: always through the host, never committed directly) and
    /// shows the card.
    fn preview_import(&mut self, params: Value, source: String, copies: usize) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let cmd = AnyCommand {
            name: COMMAND.to_owned(),
            params: params.clone(),
        };
        let submit = Submit::from(self.origin());
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
        let (created, skipped) = result
            .as_ref()
            .ok()
            .and_then(|p| parts_import_counts(p.outcome().note.as_deref()?))
            .unwrap_or((0, 0));
        let result = result.map_err(|e| e.to_string());
        let edit = PendingEdit {
            row: String::new(),
            kind: "part".to_owned(),
            route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
            path: String::new(),
            targets: Vec::new(),
            leaf: None,
            base: None,
            name: COMMAND.to_owned(),
            params: params.clone(),
            typed: String::new(),
            label: source.clone(),
        };
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params,
                name: COMMAND.to_owned(),
                result,
                derived,
            },
            edit,
            targets: copies,
            changes: created,
            reason: key!("reason-parts-import"),
            origin: CardOrigin::Import {
                source,
                created,
                skipped,
            },
        });
        Outcome::Done
    }

    /// Apply on an import card: a fresh card commits one transaction; a stale one previews the same copies again and
    /// shows a fresh card (a second Apply).
    pub(super) fn apply_import_card(&mut self, card: Card) -> Outcome {
        let Some(rev) = self.revision() else {
            return Outcome::Done;
        };
        let CardOrigin::Import {
            source,
            created,
            skipped,
        } = card.origin.clone()
        else {
            return Outcome::Done;
        };
        if let Err(message) = &card.preview.result {
            return Outcome::Failed {
                message: message.clone(),
            };
        }
        if card.preview.base_rev == rev {
            let Card { preview, edit, .. } = card;
            let outcome = self.commit_kept(preview, &edit.name, &edit.params, None);
            if outcome == Outcome::Done {
                let key = if created == 1 {
                    key!("banner-part-imported")
                } else {
                    key!("banner-parts-imported")
                };
                // The applied import is a toast with Undo, not a banner over the tab title.
                let text = strings::format(key, &[&created.to_string(), &skipped.to_string(), &source]);
                self.notify(text, true);
            }
            return outcome;
        }
        // Stale: the same copies, previewed again on the current revision.
        self.preview_import(card.edit.params, source, card.targets)
    }
}
