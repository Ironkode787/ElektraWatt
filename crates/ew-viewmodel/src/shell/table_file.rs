// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.08 part 3 (D14, ruling of ID-1224): a navigator table's export to a file and the round trip's import from one,
//! in the table mode of the focused navigator, or of the one navigator shown in it (ID-1231).
//!
//! **The table** (ruling 3): `Session::table_export` of the view's key, cut to the rows on show (matched by row ID, in
//! the order on show) and the columns on show (matched by column key, in their order); the writer keeps the hidden `id`
//! column and the `_ew` sheets, so the file re-imports. The key is `nav:<word>`, or `nav:strips/<strip>` while every
//! row on show sits under one strip of the Terminal strips navigator, as `ew export table strips --strip` names it.
//!
//! **The export** is a job ([`crate::jobs::JobKind::Table`]) under the PDF export's file rule and refusals; the
//! **import** reads the file (the reader's `Limits`) and opens [`CardOrigin::TableImport`] in the Preview tab, whose
//! Apply commits the import's one `fields.set_many` (`Session::apply_table_import`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_api::output::{
    CsvOptions, ImportTableOptions, NavKind, NavRole, Preset, TABLE_NAVIGATORS, Table, TableApplyError,
    TableImportError, TableRow, with_base_digest,
};
use ew_api::{AnyCommand, Submit};
use serde_json::{Map, Value};

use super::jobs::{absolute_target, bool_param, inside};
use super::navigation::bump;
use super::{Mode, Outcome, Prompt, RecordedCommand, ShellModel, failed, string_param};
use crate::jobs::TableFile;
use crate::key;
use crate::layout::PaneKind;
use crate::properties::{Card, CardOrigin, EditRoute, FieldReadOnly, PendingEdit, TableImportCard};
use crate::strings;
use crate::strings::Key;
use crate::tools::PreviewState;

/// The command a round trip commits.
const COMMAND: &str = "fields.set_many";

/// Whether `path` names a CSV file (its extension, any case).
fn is_csv(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"))
}

/// A whole-file refusal of an import in words (no ID, no option name).
fn import_error(e: &TableImportError) -> String {
    match e {
        TableImportError::Read(why) => strings::format(key!("error-table-import-read"), &[why]),
        TableImportError::Format(_) => strings::text(key!("error-table-import-format")).to_owned(),
        TableImportError::OtherProject(_) => strings::text(key!("error-table-import-project")).to_owned(),
        TableImportError::CsvNoTable | TableImportError::NoTable | TableImportError::Table(_) => {
            strings::text(key!("error-table-import-table")).to_owned()
        }
        TableImportError::NoIdColumn => strings::text(key!("error-table-import-id")).to_owned(),
        TableImportError::TooManyCells(n) => {
            strings::format(key!("error-table-import-many"), &[&n.to_string()])
        }
        TableImportError::Panicked(file) => strings::format(key!("error-table-import-panicked"), &[file]),
    }
}

impl ShellModel {
    /// The navigator Export table… and Import table… read, for their predicate and their actions alike (ID-1224 ruling
    /// 2, ID-1231): the focused one when it is in table mode (the Messages pane, focused, too: its action refuses as
    /// `navigator.table` does), else the one navigator shown in table mode (in front in its pane, not behind another
    /// tab); with several shown and none of them focused, the reason says to click one. A table without a file form is
    /// refused.
    pub(crate) fn table_file_nav(&self) -> Result<NavKind, Key<'static>> {
        let in_table = |k: NavKind| self.generic(k).is_some_and(|n| n.table_mode().is_some());
        let in_front = |k: NavKind| {
            let pane = match k {
                NavKind::Pages => PaneKind::Pages,
                k => PaneKind::Navigator(k),
            };
            self.layout().shown_in_place_of(pane) == Some(pane)
        };
        let kind = match self.focused_navigator() {
            Some(k) if k == NavKind::Messages || in_table(k) => k,
            _ => {
                let pages = self.pages_table.is_some().then_some(NavKind::Pages);
                let shown: Vec<NavKind> = pages
                    .into_iter()
                    .chain(self.open_navigators())
                    .filter(|k| in_table(*k) && in_front(*k))
                    .collect();
                match shown.as_slice() {
                    [k] => *k,
                    [] => return Err(key!("reason-table-off")),
                    _ => return Err(key!("reason-table-which")),
                }
            }
        };
        if kind != NavKind::Messages && !TABLE_NAVIGATORS.contains(&kind) {
            return Err(key!("reason-table-no-file"));
        }
        Ok(kind)
    }

    /// The navigator whose table the two actions read ([`ShellModel::table_file_nav`]); the Messages pane refuses as
    /// `navigator.table` does.
    fn file_table_kind(&self) -> Result<NavKind, Outcome> {
        let kind = self
            .table_file_nav()
            .map_err(|reason_key| Outcome::Refused { reason_key })?;
        if kind == NavKind::Messages {
            return Err(failed(key!("error-nav-no-table"), &[kind.word()]));
        }
        Ok(kind)
    }

    /// The table key of navigator `kind`'s view, the node keys of its rows on show and its column keys on show (ruling
    /// 3).
    fn shown_table(&self, kind: NavKind) -> Option<(String, Vec<String>, Vec<String>)> {
        let n = self.generic(kind)?;
        let mode = n.table_mode()?;
        let table = n.table()?;
        let rows: Vec<u32> = n.visible().iter().map(|r| r.node).collect();
        let parent = |i: &u32| table.nodes[*i as usize].parent;
        let strip = match rows.first().map(parent) {
            Some(Some(p)) if kind == NavKind::Strips && rows.iter().all(|i| parent(i) == Some(p)) => {
                let node = &table.nodes[p as usize];
                (node.role == NavRole::Strip)
                    .then(|| node.records.first())
                    .flatten()
            }
            _ => None,
        };
        let key = match strip {
            Some(id) => format!("nav:strips/{id}"),
            None => format!("nav:{}", kind.word()),
        };
        let row_keys = rows
            .iter()
            .map(|i| table.nodes[*i as usize].key.clone())
            .collect();
        let columns = mode
            .shown_columns(table)
            .into_iter()
            .map(|c| table.columns[c].key.clone())
            .collect();
        Some((key, row_keys, columns))
    }

    /// The table of navigator `kind`'s view, cut to its rows and columns on show (ruling 3).
    fn restricted_table(&self, kind: NavKind) -> Result<Table, Outcome> {
        let (Some((key, rows, columns)), Some(project)) = (self.shown_table(kind), &self.project) else {
            return Err(Outcome::Refused {
                reason_key: key!("reason-table-off"),
            });
        };
        let full = project
            .session
            .table_export(&key)
            .map_err(|_| failed(key!("error-nav-no-export"), &[kind.word()]))?;
        let picked: Vec<usize> = columns
            .iter()
            .filter_map(|k| full.columns.iter().position(|c| c.key == *k))
            .collect();
        let by_id: BTreeMap<&str, &TableRow> = full
            .rows
            .iter()
            .filter_map(|r| Some((r.id.as_deref()?, r)))
            .collect();
        let rows = rows
            .iter()
            .filter_map(|k| by_id.get(k.as_str()))
            .map(|r| TableRow {
                id: r.id.clone(),
                heading: r.heading,
                cells: picked.iter().filter_map(|c| r.cells.get(*c).cloned()).collect(),
            })
            .collect();
        Ok(Table {
            name: full.name.clone(),
            key: full.key.clone(),
            columns: picked.iter().map(|c| full.columns[*c].clone()).collect(),
            rows,
        })
    }

    /// `navigator.export_table {path?, format?, preset?, replace?, wait?}`: the focused navigator's table on show as a
    /// job writing `path` (XLSX, or CSV with `format: csv` or a `.csv` name) through its hidden sibling. A CSV takes
    /// the preset `excel-en` unless `preset` names another; `ew` writes no `id` column and is refused (ID-1252).
    pub(crate) fn act_export_table(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.file_table_kind() {
            Ok(k) => k,
            Err(e) => return e,
        };
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => absolute_target(PathBuf::from(path)),
            // The native save dialog asks about an existing file itself, so its answer carries `replace`.
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                return self.need_path("navigator.export_table", Prompt::SaveTable, "path", &rest);
            }
        };
        let (replace, wait) = match (bool_param(p, "replace", false), bool_param(p, "wait", true)) {
            (Ok(r), Ok(w)) => (r, w),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let csv = match string_param(p, "format") {
            Err(e) => return e,
            Ok(Some(f)) => f == "csv",
            Ok(None) => is_csv(&path),
        };
        let preset = match string_param(p, "preset") {
            Err(e) => return e,
            Ok(None) => None,
            Ok(Some(_)) if !csv => return failed(key!("error-table-preset-csv"), &[]),
            Ok(Some(name)) => match Preset::parse(name) {
                Some(Preset::Ew) => {
                    return Outcome::Refused {
                        reason_key: key!("reason-table-preset-ew"),
                    };
                }
                Some(preset) => Some(preset),
                None => return failed(key!("error-table-preset"), &[name]),
            },
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return failed(key!("error-bad-param"), &["path"]);
        };
        if self.jobs.running() > 0 {
            return Outcome::Refused {
                reason_key: key!("reason-export-running"),
            };
        }
        if inside(&project.dir, &path) {
            return Outcome::Refused {
                reason_key: key!("reason-export-into-project"),
            };
        }
        if path.exists() && !replace {
            return Outcome::Refused {
                reason_key: key!("reason-file-exists"),
            };
        }
        let table = match self.restricted_table(kind) {
            Ok(t) => t,
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::Done;
        };
        let rev = project.session.world().rev.0;
        let epoch = project.epoch;
        let file = if csv {
            TableFile::Csv {
                table,
                options: CsvOptions::preset(preset.unwrap_or(Preset::ExcelEn)),
            }
        } else {
            // No creation date, as `ew export table` without `SOURCE_DATE_EPOCH`; `_ew`'s `rev` is this revision.
            let meta = with_base_digest(&project.session.workbook_meta(None), std::slice::from_ref(&table));
            TableFile::Xlsx { table, meta }
        };
        let number = match self.jobs.start_table(file, path, name, rev, epoch) {
            Ok(n) => n,
            Err(_) => {
                let message =
                    strings::format(key!("error-export"), &[strings::text(key!("error-no-thread"))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        // Headless the step waits for its job unless `wait: false`; the window never waits.
        if self.mode == Mode::Headless && wait {
            return self.wait_job(number);
        }
        Outcome::Done
    }

    /// `navigator.import_table {path?}`: reads the file against the focused navigator's table (a CSV against the
    /// view's key) and opens the import's card in the Preview tab; nothing is committed until its Apply.
    pub(crate) fn act_import_table(&mut self, p: &Map<String, Value>) -> Outcome {
        let kind = match self.file_table_kind() {
            Ok(k) => k,
            Err(e) => return e,
        };
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("navigator.import_table", Prompt::OpenTable, "path", p),
        };
        let Some((key, _, _)) = self.shown_table(kind) else {
            return Outcome::Refused {
                reason_key: key!("reason-table-off"),
            };
        };
        let Some(rev) = self.revision() else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let input = match ew_api::catalog::read_input(&path, &ew_api::catalog::table_limits()) {
            Ok(i) => i,
            Err(ew_api::catalog::CatalogError::Io(name, e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return failed(key!("error-table-import-missing"), &[&name]);
            }
            Err(ew_api::catalog::CatalogError::Io(name, e)) => {
                return failed(
                    key!("error-table-import-open"),
                    &[&name, &crate::jobs::io_cause(&e)],
                );
            }
            Err(e) => return failed(key!("error-table-import-read"), &[&e.to_string()]),
        };
        let submit = Submit::from(self.origin());
        #[cfg(feature = "testkit")]
        let fault = self.jobs.hooks.panic_at == Some("table-read");
        let calls = &self.calls;
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        let options = ImportTableOptions {
            table: Some(key),
            ..ImportTableOptions::default()
        };
        let import = match ew_api::output::guard_table_read(input.name(), || {
            #[cfg(feature = "testkit")]
            if fault {
                panic!("a test fault in the table read");
            }
            project.session.table_import(input.bytes(), &options)
        }) {
            Ok(i) => i,
            Err(e) => {
                return Outcome::Failed {
                    message: import_error(&e),
                };
            }
        };
        // The Preview tab's lists come from the command's preview; an import without one previews nothing.
        let (params, result, derived) = match &import.command {
            Some(cmd) => {
                bump(calls, |c| c.preview_with += 1);
                let result = project.session.preview_with(cmd, submit);
                let derived = match &result {
                    Ok(p) => project.session.derived_diff(p),
                    Err(_) => Default::default(),
                };
                (cmd.params.clone(), result.map_err(|e| e.to_string()), derived)
            }
            None => (
                Value::Null,
                Err(strings::text(key!("table-nothing")).to_owned()),
                Default::default(),
            ),
        };
        let changes = import.counts().changed;
        let source = input.name().to_owned();
        self.props.card = Some(Card {
            preview: PreviewState {
                base_rev: rev,
                command: params.clone(),
                name: COMMAND.to_owned(),
                result,
                derived,
            },
            edit: PendingEdit {
                row: String::new(),
                kind: String::new(),
                route: EditRoute::ReadOnly(FieldReadOnly::NoCommand),
                path: String::new(),
                targets: Vec::new(),
                leaf: None,
                base: None,
                name: COMMAND.to_owned(),
                params,
                typed: String::new(),
                label: source.clone(),
            },
            targets: changes,
            changes,
            reason: key!("reason-table-import"),
            origin: CardOrigin::TableImport(Box::new(TableImportCard { source, import })),
        });
        Outcome::Done
    }

    /// Apply on a table import card (ruling 6): the import's one `fields.set_many`, one undo step; nothing to apply
    /// closes the card; a project changed since the read is refused in words and the card stays.
    pub(super) fn apply_table_import_card(&mut self, card: Card) -> Outcome {
        let CardOrigin::TableImport(c) = &card.origin else {
            return Outcome::Done;
        };
        let submit = Submit::from(self.origin());
        let Some(project) = &mut self.project else {
            return Outcome::Done;
        };
        match project.session.apply_table_import(&c.import, submit) {
            Ok(None) => Outcome::Done,
            Ok(Some(_)) => {
                let changed = c.import.counts().changed;
                if let Some(AnyCommand { name, params }) = c.import.command.clone() {
                    self.record.undone.clear();
                    self.record.steps.push(RecordedCommand { cmd: name, params });
                }
                self.refresh();
                let toast = if changed == 1 {
                    key!("toast-table-imported-one")
                } else {
                    key!("toast-table-imported")
                };
                self.notify(strings::format(toast, &[&changed.to_string(), &c.source]), true);
                Outcome::Done
            }
            Err(TableApplyError::Stale(..)) => {
                self.props.card = Some(card);
                Outcome::Refused {
                    reason_key: key!("reason-table-import-stale"),
                }
            }
            Err(TableApplyError::Command(e)) => {
                let message = e.to_string();
                self.props.card = Some(card);
                Outcome::Failed { message }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SE-9 (WP-S6.04 D15): a read that panicked, under the guard the window calls it through, is the banner's S2
    /// whole, with the file's name.
    #[test]
    fn a_panicked_read_is_worded_with_the_file() {
        let e = ew_api::output::guard_table_read("parts.csv", || -> Result<(), TableImportError> {
            panic!("a hostile file")
        })
        .unwrap_err();
        assert_eq!(
            import_error(&e),
            "The table import stopped on an internal error while reading parts.csv; nothing was changed."
        );
    }
}
