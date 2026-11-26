// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Labels… and Workshop settings (doc 13 §5.4, §12). Labels… lists the label files, one per label
//! set and marker part, each with its count and a tick: every label, or the labels the rework list against a release
//! names (Labels for these changes… of the Rework list…); the format (CSV or XLSX); the export profile's columns with
//! Change… (Label file columns, a form of the labels list's columns with their headings and order, and the delimiter);
//! a line that counts the labels whose text does not fit their marker part. Export asks for a folder and writes the
//! ticked files, each whole, and a toast names the folder with Show in folder. Workshop settings shows the project's
//! Properties at its Workshop rows.

use std::path::PathBuf;

use ew_api::{LabelFileFormat, LabelScope, ReworkBase, ReworkRelease};
use serde_json::{Map, Value, json};

use super::{
    Dialog, FormDialog, FormField, FormRow, FormTable, Outcome, Prompt, ShellModel, TICK, TICKED, failed,
    string_param,
};
use crate::key;
use crate::strings;

/// Which labels the files hold.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Which {
    /// Every label of the project.
    All,
    /// Those the rework list against the release at this index of [`LabelsDialog::releases`] names.
    Release(usize),
    /// Those the rework list against an archive file names.
    File(PathBuf),
}

/// One label file the dialog offers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabelFileRow {
    /// The file's name.
    pub name: String,
    /// The label set in the window's language.
    pub set: String,
    /// The marker part's number; empty without one.
    pub marker_part: String,
    /// How many labels it holds.
    pub count: usize,
    /// Whether Export writes it.
    pub on: bool,
}

/// The Labels… dialog.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabelsDialog {
    /// The releases whose archive is found, newest first, each with its words.
    pub releases: Vec<(ReworkRelease, String)>,
    /// Which labels.
    pub which: Which,
    /// The format the files are written in.
    pub format: LabelFileFormat,
    /// The files, or why there are none, in words.
    pub files: Result<Vec<LabelFileRow>, String>,
    /// The export profile's headings, in their order.
    pub columns: Vec<String>,
    /// The labels that do not fit their marker part, each its set, its marker part and its text in words.
    pub misfits: Vec<String>,
    /// Whether the dialog lists them (Show), else counts them only.
    pub show_misfits: bool,
}

impl LabelsDialog {
    /// Whether the files are written as XLSX (else CSV).
    pub fn xlsx(&self) -> bool {
        self.format == LabelFileFormat::Xlsx
    }

    /// The scope of the session's label files.
    fn scope(&self) -> LabelScope {
        match &self.which {
            Which::All => LabelScope::All,
            Which::Release(i) => self.releases.get(*i).map_or(LabelScope::All, |(r, _)| {
                LabelScope::Changes(ReworkBase::Release(r.revision))
            }),
            Which::File(path) => LabelScope::Changes(ReworkBase::File(path.clone())),
        }
    }
}

/// The files of `d` left unticked, by name.
fn unticked(d: &LabelsDialog) -> Vec<String> {
    d.files
        .iter()
        .flatten()
        .filter(|f| !f.on)
        .map(|f| f.name.clone())
        .collect()
}

/// The key of the first Workshop row of the project's Properties.
const WORKSHOP_ROW: &str = "project/settings.workshop.ferrules";
/// Label file columns…'s action.
const COLUMNS: &str = "labels.columns";

impl ShellModel {
    /// `workshop.settings` (Output ▸ Workshop ▸ Workshop settings): the project's Properties at its Workshop rows.
    pub(crate) fn act_workshop_settings(&mut self) -> Outcome {
        match self.act_props_project() {
            Outcome::Done => {}
            other => return other,
        }
        // The project's sheet now, then the Workshop rows' group opens, where it is shut.
        self.sync_properties();
        let group = self.props.sheet.as_ref().and_then(|sheet| {
            let (section, row) = crate::properties::find_row(sheet, WORKSHOP_ROW)?;
            Some(crate::properties::group_of(section, row).word())
        });
        if let Some(word) = group {
            let mut p = Map::new();
            p.insert("kind".into(), Value::String("project".into()));
            p.insert("category".into(), Value::String(word.into()));
            p.insert("expanded".into(), Value::Bool(true));
            match self.act_props_category(&p) {
                Outcome::Done => {}
                other => return other,
            }
        }
        let mut p = Map::new();
        p.insert("to".into(), Value::String(WORKSHOP_ROW.into()));
        self.act_props_row(&p)
    }

    /// `labels.show {since?, archive?, format?}` (Output ▸ Workshop ▸ Labels…; Labels for these changes… of the Rework
    /// list… with `since` or `archive`): opens the dialog on every label, or on the labels the rework list against the
    /// release labelled `since` (or the archive file `archive`) names, as CSV unless `format` is `xlsx`; the open dialog
    /// changes what it lists.
    pub(crate) fn act_labels_show(&mut self, p: &Map<String, Value>) -> Outcome {
        let releases = self.rework_releases();
        let open = match &self.dialog {
            Some(Dialog::Labels(d)) => Some((d.which.clone(), d.format)),
            _ => None,
        };
        let open_show = matches!(&self.dialog, Some(Dialog::Labels(d)) if d.show_misfits);
        let which = match (string_param(p, "since"), string_param(p, "archive")) {
            (Err(e), _) | (_, Err(e)) => return e,
            (Ok(Some("")), _) => Which::All,
            (Ok(Some(label)), _) => match releases.iter().position(|(r, _)| r.label == label) {
                Some(i) => Which::Release(i),
                None => return failed(key!("error-rework-release"), &[label]),
            },
            (_, Ok(Some(path))) => Which::File(PathBuf::from(path)),
            _ => open.as_ref().map_or(Which::All, |(w, _)| w.clone()),
        };
        let format = match string_param(p, "format") {
            Err(e) => return e,
            Ok(Some("xlsx")) => LabelFileFormat::Xlsx,
            Ok(Some("csv")) => LabelFileFormat::Csv,
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["format"]),
            Ok(None) => open.map_or(LabelFileFormat::Csv, |(_, f)| f),
        };
        let mut d = LabelsDialog {
            releases,
            which,
            format,
            files: Ok(Vec::new()),
            columns: Vec::new(),
            misfits: Vec::new(),
            show_misfits: open_show,
        };
        let off = match &self.dialog {
            Some(Dialog::Labels(old)) => unticked(old),
            _ => Vec::new(),
        };
        self.fill_labels(&mut d, &off);
        if matches!(self.dialog, Some(Dialog::Labels(_))) {
            self.dialog = Some(Dialog::Labels(Box::new(d)));
            return Outcome::Done;
        }
        self.open_dialog(Dialog::Labels(Box::new(d)))
    }

    /// The files, the columns and the misfits of `d`, from the session; a file `off` names stays unticked.
    fn fill_labels(&self, d: &mut LabelsDialog, off: &[String]) {
        let Some(project) = &self.project else {
            return;
        };
        d.columns = project
            .session
            .label_profile()
            .0
            .into_iter()
            .map(|(_, h)| h)
            .collect();
        d.files = match project.session.label_files(&d.scope(), d.format, None) {
            Ok(files) => {
                d.misfits = files
                    .iter()
                    .flat_map(|f| {
                        let set = strings::render(&ew_api::label_set_text(f.set));
                        let part = f.marker_part.clone().unwrap_or_default();
                        f.misfits
                            .iter()
                            .map(move |t| strings::format(key!("labels-misfit"), &[&set, &part, t]))
                    })
                    .collect();
                Ok(files
                    .into_iter()
                    .map(|f| LabelFileRow {
                        on: !off.contains(&f.name),
                        set: strings::render(&ew_api::label_set_text(f.set)),
                        marker_part: f.marker_part.unwrap_or_default(),
                        count: f.count,
                        name: f.name,
                    })
                    .collect())
            }
            Err(ew_api::LabelError::Rework(e)) => Err(super::rework::reason(&e)),
            Err(e) => Err(strings::format(key!("error-export"), &[&e.to_string()])),
        };
    }

    /// `labels.misfits {on?}` (the dialog's Show and Hide): lists the labels that do not fit their marker part under
    /// their count, or counts them only; without `on` it switches.
    pub(crate) fn act_labels_misfits(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Dialog::Labels(d)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-labels-closed"));
        };
        d.show_misfits = p.get("on").and_then(Value::as_bool).unwrap_or(!d.show_misfits);
        Outcome::Done
    }

    /// `labels.tick {file, on}`: ticks or unticks the file named `file` of the open dialog.
    pub(crate) fn act_labels_tick(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "file") {
            Ok(Some(n)) => n.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["file"]),
            Err(e) => return e,
        };
        let Some(on) = p.get("on").and_then(Value::as_bool) else {
            return failed(key!("error-missing-param"), &["on"]);
        };
        let Some(Dialog::Labels(d)) = &mut self.dialog else {
            return Outcome::refused(key!("reason-labels-closed"));
        };
        match d.files.iter_mut().flatten().find(|f| f.name == name) {
            Some(f) => {
                f.on = on;
                Outcome::Done
            }
            None => failed(key!("error-bad-param"), &["file"]),
        }
    }

    /// `labels.write {dir?}` (the dialog's Export): writes the ticked files into the folder `dir`, made where it is
    /// missing, each whole; without a folder the window asks for one. A toast names the folder with Show in folder.
    pub(crate) fn act_labels_write(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(Dialog::Labels(d)) = &self.dialog else {
            return Outcome::refused(key!("reason-labels-closed"));
        };
        let wanted: Vec<String> = d
            .files
            .iter()
            .flatten()
            .filter(|f| f.on)
            .map(|f| f.name.clone())
            .collect();
        if wanted.is_empty() {
            return Outcome::refused(key!("reason-labels-none"));
        }
        let (scope, format) = (d.scope(), d.format);
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("labels.write", Prompt::LabelFolder, "dir", p),
        };
        let project = or_return!(crate::shell::gated(&self.project));
        let files = match project.session.label_files(&scope, format, None) {
            Ok(f) => f,
            Err(e) => return failed(key!("error-export"), &[&e.to_string()]),
        };
        let files: Vec<ew_api::LabelFile> = files.into_iter().filter(|f| wanted.contains(&f.name)).collect();
        if let Err(e) = ew_api::Session::write_label_files(&dir, &files) {
            return failed(key!("error-export"), &[&crate::jobs::io_cause(&e)]);
        }
        let folder = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text =
            strings::format_counted(crate::count_key!("toast-labels-written"), files.len(), &[&folder]);
        self.notify_folder(text, "edit", dir);
        Outcome::Done
    }

    /// `labels.columns {columns?, delimiter?}` (Label file columns…, the dialog's Change…): without a parameter the
    /// window opens the form: every column of the labels list, one row each in the window's words, with a tick to write
    /// it, the heading the file prints and its order, and the delimiter. Apply sets the project's export profile in
    /// one step (`project.settings.set workshop.label_profile`); the generic profile unchanged clears it. Refused in
    /// words: no column ticked, an order that is not a whole number, a column ticked without a heading.
    pub(crate) fn act_labels_columns(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let (profile, delimiter_now) = project.session.label_profile();
        let words = [
            strings::text(key!("list-columns-print")).to_owned(),
            strings::text(key!("list-columns-heading")).to_owned(),
            strings::text(key!("list-columns-order")).to_owned(),
        ];
        let label =
            |c: ew_api::LabelColumn| -> String { ew_api::label_column_words(c, Some(strings::locale())) };
        if p.is_empty() && !self.in_form_of(COLUMNS) {
            self.labels_under = match self.dialog.take() {
                Some(Dialog::Labels(d)) => Some(d),
                other => {
                    self.dialog = other;
                    None
                }
            };
            let rows = ew_api::LabelColumn::ALL
                .into_iter()
                .map(|c| {
                    let at = profile.iter().position(|(k, _)| k == c.key());
                    FormRow {
                        key: label(c),
                        cells: vec![
                            if at.is_some() { TICKED } else { "" }.to_owned(),
                            at.map(|i| profile[i].1.clone()).unwrap_or_default(),
                            at.map(|i| (i + 1).to_string()).unwrap_or_default(),
                        ],
                    }
                })
                .collect();
            return self.open_dialog(Dialog::Form(FormDialog {
                action: COLUMNS,
                title_key: key!("labels-columns-title"),
                ok_key: key!("dialog-apply"),
                fields: vec![FormField {
                    label: None,
                    name: "delimiter".into(),
                    label_key: key!("field-project-settings-workshop-label-profile-delimiter"),
                    choices: &super::choices::DELIMITER,
                    options: Vec::new(),
                    value: delimiter_now.clone(),
                }],
                error: None,
                table: Some(FormTable {
                    name: "columns",
                    heading_key: key!("list-columns-table"),
                    fixed: true,
                    key_label: key!("list-columns-column"),
                    add_key: key!("form-value-set-add"),
                    columns: vec![
                        (words[0].clone(), TICK.to_owned()),
                        (words[1].clone(), String::new()),
                        (words[2].clone(), String::new()),
                    ],
                    rows,
                }),
            }));
        }
        let table = p
            .get("columns")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut chosen: Vec<(u32, usize, (String, String))> = Vec::new();
        for (i, c) in ew_api::LabelColumn::ALL.into_iter().enumerate() {
            let cells = table
                .get(i)
                .and_then(|r| r.get("values"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let cell = |w: &str| {
                cells
                    .get(w)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            };
            if cell(&words[0]) != TICKED {
                continue;
            }
            let heading = cell(&words[1]);
            if heading.is_empty() {
                let message = strings::format(key!("error-label-heading"), &[&label(c)]);
                return self.form_failed(COLUMNS, message);
            }
            let order = cell(&words[2]);
            let place = if order.is_empty() {
                u32::MAX
            } else {
                match order.parse::<u32>() {
                    Ok(o) => o,
                    Err(_) => {
                        let message = strings::format(key!("error-list-order"), &[&label(c), &order]);
                        return self.form_failed(COLUMNS, message);
                    }
                }
            };
            chosen.push((place, i, (c.key().to_owned(), heading)));
        }
        if chosen.is_empty() && !table.is_empty() {
            return self.form_failed(COLUMNS, strings::text(key!("error-list-no-column")).to_owned());
        }
        chosen.sort_by_key(|(place, i, _)| (*place, *i));
        let columns: Vec<(String, String)> = if table.is_empty() {
            profile
        } else {
            chosen.into_iter().map(|(_, _, c)| c).collect()
        };
        // The delimiter the profile has stays as it is, one of a newer format too; another is one this build knows.
        let delimiter = match string_param(p, "delimiter") {
            Err(e) => return e,
            Ok(Some(d)) if d == delimiter_now => None,
            Ok(Some(d)) if ew_api::Delimiter::NAMES.contains(&d) => Some(d.to_owned()),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["delimiter"]),
            Ok(None) => None,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let value = match project.session.label_profile_value(&columns, delimiter.as_deref()) {
            Ok(v) => v,
            Err(_) => return failed(key!("error-bad-param"), &["delimiter"]),
        };
        let cmd = ew_api::AnyCommand {
            name: "project.settings.set".to_owned(),
            params: json!({"member": "workshop.label_profile", "value": value}),
        };
        let out = match self.run_commit(&cmd) {
            Outcome::Done => {
                self.notify(strings::text(key!("toast-label-columns")).to_owned(), true);
                Outcome::Done
            }
            Outcome::Failed { message } => return self.form_failed(COLUMNS, message),
            other => other,
        };
        let out = self.form_done(COLUMNS, out);
        // The Labels… dialog the form was opened from shows again, with the new columns.
        if out == Outcome::Done
            && self.dialog.is_none()
            && let Some(mut d) = self.labels_under.take()
        {
            let off = unticked(&d);
            self.fill_labels(&mut d, &off);
            self.dialog = Some(Dialog::Labels(d));
        }
        out
    }
}
