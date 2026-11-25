// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC's files and previews (doc 10 §4, §6).
//!
//! - **PLC exchange**: Export… is a form of format and version, stations and document date that writes the file whole
//!   or not at all (`Session::plc_export`) and records it (`plc.record_exchange`); Import… reads an AML file or a TIA
//!   tag workbook into `Dialog::PlcImport`, the preview by tab (changed, conflicts, removed, parking) with the choices
//!   of its conflicts and parked objects, and Apply commits `plc.apply_exchange` under them.
//! - **Instrument list** (`plc.instrument_list`) and the **addressing preview** (`plc.preview`): a card in the Preview
//!   tab, which Apply commits as one undo step.
//!
//! Not here: the Export… form's fields are in `shell/creation.rs`; the station view in `shell/station.rs`.

use serde_json::{Map, Value, json};

use super::{Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::strings::{self, Key};

impl ShellModel {
    /// `plc.preview {cmd, params}` (FR-PLC-014, FR-PLC-015): the previewed PLC addressing command `cmd` (`plc.readdress`,
    /// `plc.convert_manual_to_auto`, `plc.freeze`) as a card in the Preview tab, whose list of changes is the command's
    /// notices (every channel whose address moves, old → new); Apply commits it as one undo step.
    pub(crate) fn act_plc_preview(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "cmd") {
            Ok(Some(n)) if ["plc.readdress", "plc.convert_manual_to_auto", "plc.freeze"].contains(&n) => {
                n.to_owned()
            }
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["cmd"]),
            Ok(None) => return failed(key!("error-missing-param"), &["cmd"]),
            Err(e) => return e,
        };
        let params = p.get("params").cloned().unwrap_or_else(|| json!({}));
        self.preview_bulk(&name, params)
    }

    /// `plc.instrument_list {config, path?}` (FR-PLC-050, doc 10 §4.7): a configuration project row's Import instrument
    /// list…: the instrument list (a CSV or the first sheet of a workbook) as `plc.apply_instrument_list`, previewed in
    /// the Preview tab; Apply commits it as one undo step. In the window a missing path asks for the table.
    pub(crate) fn act_plc_instrument_list(&mut self, p: &Map<String, Value>) -> Outcome {
        let config = or_return!(self.record_param(p, "config"));
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.instrument_list", crate::shell::Prompt::OpenTable, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = or_return!(read_exchange_file(&path, &file));
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let config = config.to_string();
        let cmd = match session.instrument_list(Some(&config), &bytes) {
            Ok(cmd) => cmd,
            Err(e) => return failed(key!("error-plc-import-file"), &[&file, &e.to_string()]),
        };
        self.preview_bulk(&cmd.name, cmd.params)
    }
}

/// The command a preview was made of, printed by its name only (its parameters hold the whole file read).
#[derive(Clone, PartialEq, Eq)]
pub struct PreviewedCommand(pub std::sync::Arc<ew_api::AnyCommand>);

impl std::fmt::Debug for PreviewedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0.name)
    }
}

/// The tabs of the PLC import's preview (doc 10 §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportTab {
    /// The differing fields the merge applies, keeps, rejects or leaves pending.
    Changed,
    /// The fields both sides changed: each takes the file's value or keeps the project's.
    Conflicts,
    /// The objects of the last exchange the file no longer holds.
    Removed,
    /// The file's objects without a counterpart: a creatable one can be created unplaced.
    Parking,
}

impl ImportTab {
    /// The tabs in order.
    pub const ALL: [ImportTab; 4] = [
        ImportTab::Changed,
        ImportTab::Conflicts,
        ImportTab::Removed,
        ImportTab::Parking,
    ];

    /// The tab's word (`plc.import_tab`'s `tab`).
    pub fn word(self) -> &'static str {
        match self {
            ImportTab::Changed => "changed",
            ImportTab::Conflicts => "conflicts",
            ImportTab::Removed => "removed",
            ImportTab::Parking => "parking",
        }
    }
}

/// One differing field of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportChange {
    /// Its key in the resolutions.
    pub id: String,
    /// The object and the field in words (`channel Q0.1 · symbol`).
    pub text: String,
    /// The project's value, and the file's.
    pub ours: String,
    /// The file's value.
    pub theirs: String,
    /// What the merge makes of it.
    pub state: ew_api::plc_exchange::State,
    /// The side Apply takes without a choice: `ours` or `theirs`.
    pub default: &'static str,
    /// Why the file's value cannot be taken, in words; `None` when it can.
    pub refused: Option<String>,
}

/// One parked object of the import's preview.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportParked {
    /// Its ID in the file: its key in the resolutions.
    pub id: String,
    /// Its kind and name in words.
    pub text: String,
    /// Whether several objects of the project match it (else none does).
    pub ambiguous: bool,
    /// Whether "create unplaced" applies.
    pub creatable: bool,
}

/// The PLC import's preview (`Dialog::PlcImport`): the file, its tab on show, the plan in words, and the choices.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlcImportCard {
    /// The file's name.
    pub file: String,
    /// The file's path, which the preview is made of again when "the file is complete" changes.
    pub path: String,
    /// Whether the file is complete (doc 10 §4.4): an object of the last exchange it lacks is removed.
    pub complete: bool,
    /// Why Apply would be refused (its prevent rules, tried on the preview), in words; `None` when it commits.
    pub refusal: Option<String>,
    /// The `plc.apply_exchange` the preview was made of, with every option it was made with (`complete`); Apply
    /// commits it under the choices, and the file is neither read nor merged again here.
    pub apply: PreviewedCommand,
    /// The tab on show.
    pub tab: ImportTab,
    /// The differing fields (not conflicts).
    pub changes: Vec<ImportChange>,
    /// The conflicts.
    pub conflicts: Vec<ImportChange>,
    /// The removed objects in words.
    pub removed: Vec<String>,
    /// The parked objects.
    pub parking: Vec<ImportParked>,
    /// The choices: a change's key or a parked object's ID to `ours`, `theirs` or `create`.
    pub choices: std::collections::BTreeMap<String, String>,
}

impl PlcImportCard {
    /// The rows of tab `tab`.
    pub fn count(&self, tab: ImportTab) -> usize {
        match tab {
            ImportTab::Changed => self.changes.len(),
            ImportTab::Conflicts => self.conflicts.len(),
            ImportTab::Removed => self.removed.len(),
            ImportTab::Parking => self.parking.len(),
        }
    }
}

/// The bytes of the exchange file or instrument list at `path` (named `file`), read through the importers' input limit:
/// a file larger than the limit, or one that cannot be read, is refused in words.
fn read_exchange_file(path: &std::path::Path, file: &str) -> Result<Vec<u8>, Outcome> {
    match super::drawing_import::read_limited(path) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => {
            let mib = (ew_api::plc_exchange::max_file_bytes() >> 20).to_string();
            Err(failed(key!("error-plc-import-too-large"), &[file, &mib]))
        }
        Err(e) => Err(failed(
            key!("error-plc-import-file"),
            &[file, &crate::jobs::read_cause(&e)],
        )),
    }
}

/// The PLC import's card of the preview `import` of the file `file`: its rows in the window's words.
fn plc_import_card(
    file: String,
    import: ew_api::plc_exchange::PlcImport,
    refusal: Option<&(ew_api::CmdError, Vec<String>)>,
) -> PlcImportCard {
    let words = |v: &Option<String>| v.clone().unwrap_or_default();
    // Apply's refusal in the window's words; the rows of the changes it concerns say so.
    let why = refusal.map(|(e, _)| match super::tools::command_failed(e, &Value::Null) {
        Outcome::Failed { message } => message,
        _ => e.to_string(),
    });
    let named: &[String] = refusal.map_or(&[], |(_, changes)| changes.as_slice());
    let (conflicts, changes): (Vec<_>, Vec<_>) = import
        .plan
        .changes
        .iter()
        .map(|c| {
            let field = plc_field_text(&c.object, &c.field);
            let refused = (!c.acceptable).then(|| {
                // A field of a group the file cannot take is a comment of the other form (the command's reason).
                let key = if c.group.is_some() {
                    key!("refusal-plc-exchange-language")
                } else {
                    key!("refusal-plc-exchange-not-acceptable")
                };
                strings::sentence_start(&strings::format(key, &[&field, &c.subject])).into_owned()
            });
            let refused = refused.or_else(|| why.clone().filter(|_| named.contains(&c.id)));
            ImportChange {
                id: c.id.clone(),
                text: strings::format(key!("plc-import-row"), &[&c.subject, &field]),
                ours: words(&c.ours),
                theirs: words(&c.theirs),
                state: c.state,
                default: match c.default {
                    ew_api::plc_exchange::Choice::Ours => "ours",
                    ew_api::plc_exchange::Choice::Theirs => "theirs",
                },
                refused,
            }
        })
        .partition(|c| c.state == ew_api::plc_exchange::State::Conflict);
    let object = |o: &ew_api::plc_exchange::ExchangeObject, name: &str| {
        strings::format(key!("plc-import-object"), &[&plc_object_text(o), name])
    };
    PlcImportCard {
        file,
        path: String::new(),
        complete: false,
        refusal: why,
        apply: PreviewedCommand(std::sync::Arc::new(import.apply)),
        tab: ImportTab::Changed,
        changes,
        conflicts,
        removed: import
            .plan
            .removed
            .iter()
            .map(|(o, _, name)| object(o, name))
            .collect(),
        parking: import
            .plan
            .parking
            .iter()
            .map(|p| ImportParked {
                id: p.id.clone(),
                text: object(&p.object, &p.name),
                ambiguous: p.ambiguous,
                creatable: p.creatable,
            })
            .collect(),
        choices: std::collections::BTreeMap::new(),
    }
}

/// The kind of an exchanged object in the window's words: the text of its key (`ew_plc::object_key`).
pub fn plc_object_text(object: &ew_api::plc_exchange::ExchangeObject) -> String {
    strings::text(Key::run_time(ew_api::plc_exchange::object_key(object))).to_owned()
}

/// A field of an exchanged object in the window's words: the text of its key (`ew_plc::field_name`), a comment in a
/// language with the language; a field this version does not name as it is written.
pub fn plc_field_text(object: &ew_api::plc_exchange::ExchangeObject, field: &str) -> String {
    use ew_api::plc_exchange::FieldName;
    match ew_api::plc_exchange::field_name(object, field) {
        FieldName::Key(key) => strings::text(Key::run_time(key)).to_owned(),
        FieldName::Comment(lang) => strings::format(key!("plc-field-comment-in"), &[lang]),
        FieldName::Unknown(field) => field.to_owned(),
    }
}

/// The PLC export formats PLC exchange… offers: (word, the format, its name); product names, the same in every
/// language.
pub const PLC_FORMATS: [(&str, &str); 4] = [
    ("aml-1.4.0", "AutomationML AR APC 1.4.0"),
    ("aml-1.2.0", "AutomationML AR APC 1.2.0"),
    ("tia-xlsx", "TIA Portal PLC tags (XLSX)"),
    ("plcopen", "PLCopen XML (CODESYS)"),
];

/// The exchange format of a word of [`PLC_FORMATS`].
pub(super) fn plc_format(word: &str) -> Option<ew_api::plc_exchange::PlcFormat> {
    use ew_api::plc_exchange::{ApcVersion, PlcFormat};
    match word {
        "aml-1.4.0" => Some(PlcFormat::Aml(ApcVersion::V1_4_0)),
        "aml-1.2.0" => Some(PlcFormat::Aml(ApcVersion::V1_2_0)),
        "tia-xlsx" => Some(PlcFormat::TiaXlsx),
        "plcopen" => Some(PlcFormat::Plcopen),
        _ => None,
    }
}

impl ShellModel {
    /// `plc.import {path?, complete?}` (doc 10 §4.4): reads an AML file or a TIA tag workbook and opens its preview
    /// against the project's configuration project; nothing changes until Apply. In the window a missing path asks for
    /// the file.
    pub(crate) fn act_plc_import(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => std::path::PathBuf::from(path),
            Ok(None) => {
                return self.need_path("plc.import", crate::shell::Prompt::OpenPlcExchange, "path", p);
            }
        };
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let complete = p.get("complete").and_then(Value::as_bool).unwrap_or(false);
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let options = ew_api::plc_exchange::PlcOptions {
            complete,
            ..Default::default()
        };
        let reader = match session.plc_import_read(&options) {
            Ok(r) => r,
            Err(e) => return failed(key!("error-plc-import"), &[&e.to_string()]),
        };
        // The file read and merged as a job, the preview opened at its end.
        let name = file.clone();
        self.start_read(&name, move |_| {
            let import =
                read_exchange_file(&path, &file).map(|bytes| reader.run(&bytes).map_err(|e| e.to_string()));
            Ok(
                Box::new(move |s: &mut ShellModel| s.plc_import_read(path, file, complete, import))
                    as super::read_jobs::Finish,
            )
        })
    }

    /// The end of a PLC import's read: its preview, or why the file was not read.
    fn plc_import_read(
        &mut self,
        path: std::path::PathBuf,
        file: String,
        complete: bool,
        import: Result<Result<ew_api::plc_exchange::PlcImport, String>, Outcome>,
    ) -> Outcome {
        let import = match import {
            Err(outcome) => return outcome,
            Ok(Err(why)) => return failed(key!("error-plc-import"), &[&why]),
            Ok(Ok(i)) => i,
        };
        // Apply tried on the preview (its prevent rules run), so a start the file gives that overlaps another is
        // refused here already, its row marked, rather than at Apply.
        let refusal = self
            .project
            .as_mut()
            .and_then(|project| project.session.plc_import_refusal(&import));
        let mut card = plc_import_card(file, import, refusal.as_ref());
        card.path = path.to_string_lossy().into_owned();
        card.complete = complete;
        if card.changes.is_empty() && !card.conflicts.is_empty() {
            card.tab = ImportTab::Conflicts;
        }
        self.open_dialog(crate::shell::Dialog::PlcImport(Box::new(card)))
    }

    /// `plc.import_tab {tab}`: the tab the import's preview shows.
    pub(crate) fn act_plc_import_tab(&mut self, p: &Map<String, Value>) -> Outcome {
        let tab = match string_param(p, "tab") {
            Ok(Some(w)) => match ImportTab::ALL.into_iter().find(|t| t.word() == w) {
                Some(t) => t,
                None => return failed(key!("error-bad-param"), &["tab"]),
            },
            Ok(None) => return failed(key!("error-missing-param"), &["tab"]),
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                card.tab = tab;
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_choose {id, choice?}`: a conflict's or a parked object's choice (`ours`, `theirs`, `create`; absent:
    /// the default again).
    pub(crate) fn act_plc_import_choose(&mut self, p: &Map<String, Value>) -> Outcome {
        let id = match string_param(p, "id") {
            Ok(Some(i)) => i.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["id"]),
            Err(e) => return e,
        };
        let choice = match string_param(p, "choice") {
            Ok(Some(c)) if ["ours", "theirs", "create"].contains(&c) => Some(c.to_owned()),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["choice"]),
            Ok(None) => None,
            Err(e) => return e,
        };
        match &mut self.dialog {
            Some(crate::shell::Dialog::PlcImport(card)) => {
                match choice {
                    Some(c) => card.choices.insert(id, c),
                    None => card.choices.remove(&id),
                };
                Outcome::Done
            }
            _ => Outcome::refused(key!("reason-plc-import-closed")),
        }
    }

    /// `plc.import_apply`: the import's `plc.apply_exchange` under its choices, one undo step; the preview closes and a
    /// toast says what changed.
    pub(crate) fn act_plc_import_apply(&mut self) -> Outcome {
        let Some(crate::shell::Dialog::PlcImport(card)) = self.dialog.clone() else {
            return Outcome::refused(key!("reason-plc-import-closed"));
        };
        let mut apply = (*card.apply.0).clone();
        apply.params["resolutions"] = json!(card.choices);
        let out = self.run_commit(&apply);
        if out == Outcome::Done {
            self.dialog = None;
            self.notify(strings::format(key!("toast-plc-import"), &[&card.file]), true);
        }
        out
    }

    /// `plc.export {format?, stations?, date?, path?, replace?}` (doc 10 §4.3, §4.5, §4.6): the configuration project
    /// exported in the format (`aml-1.4.0`, `aml-1.2.0`, `tia-xlsx`, `plcopen`) for the stations (tags, comma
    /// separated; empty: every station) at the document date (`YYYY-MM-DD`; empty: the model's), written whole or
    /// not at all, then recorded with `plc.record_exchange`. In the window a missing format opens the form and a missing
    /// path asks for the file.
    pub(crate) fn act_plc_export_file(&mut self, p: &Map<String, Value>, format: &str) -> Outcome {
        let Some(fmt) = plc_format(format) else {
            return failed(key!("error-bad-param"), &["format"]);
        };
        let stations: Vec<String> = string_param(p, "stations")
            .ok()
            .flatten()
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(|s| if s.contains(':') { s } else { format!("tag:{s}") })
            .collect();
        let date = match string_param(p, "date")
            .ok()
            .flatten()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            None => None,
            Some(d) => match d.parse::<ew_api::plc_exchange::Date>() {
                Ok(d) => Some(d),
                Err(_) => return failed(key!("error-plc-export-date"), &[d]),
            },
        };
        // The form's values are checked before the file is asked for, so a value refused keeps the form.
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => super::jobs::absolute_target(std::path::PathBuf::from(path)),
            Ok(None) => {
                let mut rest = p.clone();
                rest.insert("replace".to_owned(), Value::Bool(true));
                self.dialog = None;
                let prompt = crate::shell::Prompt::SavePlcExchange(fmt.extension());
                return self.need_path("plc.export", prompt, "path", &rest);
            }
        };
        // An existing file is replaced only on request (the save dialog asks itself, and says so).
        if path.exists() && !or_return!(super::jobs::bool_param(p, "replace", false)) {
            return Outcome::refused(key!("reason-file-exists"));
        }
        let session = &or_return!(crate::shell::gated(&self.project)).session;
        let options = ew_api::plc_exchange::PlcOptions {
            stations,
            date,
            ..Default::default()
        };
        let export = match session.plc_export(fmt, &options) {
            Ok(e) => e,
            Err(e) => return failed(key!("error-plc-export"), &[&e.to_string()]),
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Err(e) = ew_api::file::write_replacing(&path, &export.bytes) {
            return failed(key!("error-plc-export"), &[&e.to_string()]);
        }
        let out = export
            .record
            .as_ref()
            .map_or(Outcome::Done, |record| self.run_commit(record));
        if out == Outcome::Done {
            let [stations, modules, channels, tags] = export.counts;
            let [stations, modules, channels, tags] = [
                strings::format_count(crate::count_key!("plc-count-stations"), stations),
                strings::format_count(crate::count_key!("plc-count-modules"), modules),
                strings::format_count(crate::count_key!("plc-count-channels"), channels),
                strings::format_count(crate::count_key!("plc-count-symbols"), tags),
            ];
            let mut text = strings::format(
                key!("toast-plc-export"),
                &[&name, &stations, &modules, &channels, &tags],
            );
            for w in &export.warnings {
                text = format!("{} {w}.", strings::without_stop(&text));
            }
            self.notify(text, false);
        }
        out
    }
}
