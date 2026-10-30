// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! File, Import, Parts table… (FT-410): the headless action `parts.import`.
//!
//! The action asks for a `parts.csv` (the window's file dialog, or the `path` parameter), reads it with the tables
//! beside it through `ew_api::import::read_parts_tables`, and previews the `parts.import` command through the host.
//! The command's cost is Bulk, so the inline preview card always shows: Apply on a fresh card commits one transaction
//! (one undo step) and leaves the banner "Imported 29 parts from parts.csv (0 skipped)."; a stale card previews the
//! same copies again and shows a fresh card that needs a second Apply. A refused table shows its first problem, with
//! its pointer, in the banner and opens no card. The registry's `writable` check has run before the action body
//! (`ShellModel::dispatch`).
//!
//! **The column mapping step (FT-649, WP-S1.19 §9, D11).** `parts.import {path}` first opens the mapping card on any
//! table (CSV or XLSX): the source key (pre-filled from the file name), and per column its header, a sample value and
//! the member it imports as, pre-filled from the map saved for the source key (`<config dir>/imports/<key>.toml`),
//! else from the golden dialect's headers when the file has exactly those. `parts.map` changes the key or a column's
//! member; `parts.map_import` saves the map and reads the table through `ew_api::catalog`, whose new parts go to the
//! import card above. A golden-dialect table imported with its pre-filled map unchanged is read by the golden reader
//! with its sibling tables (part functions, cores), exactly as before the step. Escape (`ui.dismiss`) closes the card.

use std::path::PathBuf;

use ew_api::catalog;
use ew_api::import::{PartsImport, parts_import_counts, read_parts_tables};
use ew_api::{AnyCommand, Origin, Submit};
use serde_json::{Map, Value};

use super::navigation::bump;
use super::{Mode, Outcome, Prompt, ShellModel, failed, string_param};
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit};
use crate::strings;
use crate::tools::PreviewState;

/// The name of the command the action dispatches.
const COMMAND: &str = "parts.import";

/// The columns of the golden dialect's `parts.csv` (`ew-import-tables`) and the member each fills in a column map:
/// the required `key`, `manufacturer` and `part_number`, then the optional ones; `key`, `test_data` and `tech_data`
/// (a `name=value;…` list) fill no member of the general importer.
const GOLDEN_COLUMNS: [(&str, Option<&str>); 9] = [
    ("key", None),
    ("manufacturer", Some("manufacturer")),
    ("part_number", Some("part_number")),
    ("kind", Some("kind")),
    ("type_designation", Some("type_designation")),
    ("description", Some("description")),
    ("unit", Some("unit")),
    ("test_data", None),
    ("tech_data", None),
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

/// One column of the mapping card (FT-649).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MapColumn {
    /// The header.
    pub header: String,
    /// The first value under it.
    pub sample: String,
    /// The member it imports as (`part_number`, `tech_data.i_n`); `None`: not imported.
    pub member: Option<String>,
}

/// The column mapping step of File ▸ Import ▸ Parts table… (FT-649), while it is open.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartsMap {
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
    /// The member choices of the column `header` (FT-649): the members without a key in declaration order, then the
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

    /// The column map of the chosen members, or the importer's message why it is not usable.
    fn column_map(&self) -> Result<catalog::ColumnMap, String> {
        let mut chosen = std::collections::BTreeSet::new();
        if let Some(twice) = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref())
            .find(|m| !chosen.insert(*m))
        {
            return Err(strings::format("parts-map-twice", &[twice]));
        }
        let pairs = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref().map(|m| (m, c.header.as_str())));
        catalog::ColumnMap::new(None, pairs).map_err(|e| e.to_string())
    }

    /// Whether Import can run: the importer takes the map.
    pub fn can_import(&self) -> bool {
        self.column_map().is_ok()
    }

    /// A member in words, as the mapping card shows it (cycle 37, window check W37; doc 16a P2): "Manufacturer",
    /// "Part number", "Description (de)", "Technical data: Nennstrom", "Free property: Farbe", with `header` the
    /// column's header. A name that is no member is shown as it is. Describe text and scripts keep the member names.
    pub fn member_text(member: &str, header: &str) -> String {
        match catalog::Member::parse(member) {
            Ok(catalog::Member::Description(Some(lang))) => {
                strings::format("parts-member-description-lang", &[&lang])
            }
            Ok(catalog::Member::TechData(_)) => strings::format("parts-member-tech-data", &[header]),
            Ok(catalog::Member::FreeProp(_)) => strings::format("parts-member-free-prop", &[header]),
            Ok(m) => strings::text(match m {
                catalog::Member::Manufacturer => "parts-member-manufacturer",
                catalog::Member::PartNumber => "parts-member-part-number",
                catalog::Member::TypeDesignation => "parts-member-type-designation",
                catalog::Member::OrderNumber => "parts-member-order-number",
                catalog::Member::Gtin => "parts-member-gtin",
                catalog::Member::Kind => "parts-member-kind",
                catalog::Member::Unit => "parts-member-unit",
                _ => "parts-member-description",
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
        let mut chosen = std::collections::BTreeSet::new();
        if let Some(c) = self
            .columns
            .iter()
            .find(|c| c.member.as_deref().is_some_and(|m| !chosen.insert(m)))
        {
            let member = c.member.as_deref().unwrap_or_default();
            return Some(strings::format(
                "parts-map-twice-text",
                &[&Self::member_text(member, &c.header)],
            ));
        }
        let pairs = self
            .columns
            .iter()
            .filter_map(|c| c.member.as_deref().map(|m| (m, c.header.as_str())));
        Some(match catalog::ColumnMap::new(None, pairs) {
            Err(catalog::MapError::Missing(m)) => {
                strings::format("parts-map-missing", &[&Self::member_text(m, "")])
            }
            Err(catalog::MapError::MixedDescription) => strings::text("parts-map-mixed").to_owned(),
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

    /// The column mapping step while it is open (FT-649).
    pub fn parts_map(&self) -> Option<&PartsMap> {
        self.parts_map.as_ref()
    }

    /// The banner and the failure of a table that cannot be read.
    fn parts_refused(&mut self, why: &str) -> Outcome {
        let message = strings::format("error-parts-import", &[why]);
        self.banner = Some(message.clone());
        Outcome::Failed { message }
    }

    /// `parts.import {path}` (FT-649): reads the table's header row and opens the column mapping step on it,
    /// pre-filled from the map saved for the file's source key, else from the golden dialect's headers when the file
    /// has exactly those. A table that cannot be read shows its problem in the banner and opens nothing.
    pub(super) fn act_parts_import(&mut self, p: &Map<String, Value>) -> Outcome {
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

    /// `parts.map {source?, column?, member?}` (FT-649): sets the source key (a map saved for it fills the members)
    /// and the member the column with header `column` imports as (`member` empty or absent: not imported).
    pub(super) fn act_parts_map(&mut self, p: &Map<String, Value>) -> Outcome {
        let (source, column, member) = match (
            string_param(p, "source"),
            string_param(p, "column"),
            string_param(p, "member"),
        ) {
            (Ok(s), Ok(c), Ok(m)) => (s.map(str::to_owned), c.map(str::to_owned), m.map(str::to_owned)),
            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => return e,
        };
        let Some(mut map) = self.parts_map.take() else {
            return Outcome::Refused {
                reason_key: "reason-no-parts-map",
            };
        };
        let outcome = (|| {
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
                    return failed("error-bad-param", &["column"]);
                };
                let member = member.map(|m| m.trim().to_owned()).filter(|m| !m.is_empty());
                if let Some(m) = &member
                    && let Err(e) = catalog::Member::parse(m)
                {
                    return Outcome::Failed {
                        message: e.to_string(),
                    };
                }
                map.columns[i].member = member;
            }
            Outcome::Done
        })();
        map.error = map.column_map().err();
        self.parts_map = Some(map);
        outcome
    }

    /// `parts.map_import` (FT-649): saves the map for the source key and reads the table through it
    /// (`ew_api::catalog`): the new parts go to the import card (`parts.import`, as before), the counts of the rows read
    /// to the toast. A golden-dialect table whose pre-filled map is unchanged is read by the golden reader with its
    /// sibling tables, as before the mapping step. Refused while the importer refuses the map (its message under the
    /// table).
    pub(super) fn act_parts_map_import(&mut self) -> Outcome {
        let Some(map) = self.parts_map.clone() else {
            return Outcome::Refused {
                reason_key: "reason-no-parts-map",
            };
        };
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
            return self.preview_import(params, source, copies.len());
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
        let Some(cmd) = cmd else {
            let message = strings::format("banner-parts-none", &[&map.file, &held, &rejected]);
            self.banner = Some(message);
            return Outcome::Done;
        };
        self.notify(
            strings::format(
                "toast-parts-read",
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
        let submit = match self.mode {
            Mode::Gui => Submit::from(Origin::Gui),
            Mode::Headless => Submit::from(Origin::CmdLog {
                file: self.origin_file.clone(),
            }),
        };
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
            reason: "reason-parts-import",
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
                    "banner-part-imported"
                } else {
                    "banner-parts-imported"
                };
                self.banner = Some(strings::format(
                    key,
                    &[&created.to_string(), &skipped.to_string(), &source],
                ));
            }
            return outcome;
        }
        // Stale: the same copies, previewed again on the current revision.
        self.preview_import(card.edit.params, source, card.targets)
    }
}
