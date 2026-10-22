// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell model (WP-S2.01 part 1, spec section 5.1): all application state of the desktop shell and its one
//! mutating entry point, [`ShellModel::dispatch`].
//!
//! The model owns the settings, the optional open project (its `Session`, directory and access mode), the current
//! page, the banner, the palette and dialog state, and the record of committed `command.run` steps. It reaches the
//! project only through `ew_api::Session`. The egui frame (`ew-ui`) and the headless runner call `dispatch` for
//! every menu item, palette entry, shortcut and dialog result and read the getters to paint or to describe, so
//! anything the headless mode cannot do the shell does not do (spec section 6).

use std::path::{Path, PathBuf};

use ew_api::output::{ExportOptions, ExportPages, PageSelection};
use ew_api::{
    Access, CmdLog, OpenError, OpenReport, Origin, ReadOnlyReason, Recovery, Session, SessionOptions,
    StoreOptions,
};
use serde_json::{Map, Value, json};

use crate::actions::{self, ACTIONS, PaletteEntry, PaletteRun};
use crate::settings::{self, Settings, UI_SCALES, scale_index};
use crate::status;
use crate::strings;

/// Who drives the shell: the window or the headless runner. They share `dispatch`; the modes differ only where a
/// person can be asked (a dialog) and a script cannot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// The window: a missing parameter becomes a prompt, unsaved changes become a dialog.
    Gui,
    /// No window: a missing parameter fails the step, unsaved changes fail it unless `discard` is `true`.
    Headless,
}

/// Where the shell keeps its files outside projects.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ShellConfig {
    /// The directory of `settings.toml`; `None` when the environment names none (settings are not persisted).
    pub config_dir: Option<PathBuf>,
    /// The directory of the session journals; `None` for the store's default (the OS state directory).
    pub state_dir: Option<PathBuf>,
    /// The application version (the store's identity and the transcript header).
    pub app_version: String,
}

impl ShellConfig {
    /// The configuration of the environment: `EW_CONFIG_DIR` when set (journals then go to its `state`
    /// subdirectory, so a test never writes outside its own directory), else the platform directories.
    pub fn from_env(app_version: &str) -> ShellConfig {
        let overridden = config_dir_overridden();
        let config_dir = settings::default_config_dir();
        let state_dir = if overridden {
            config_dir.as_ref().map(|d| d.join("state"))
        } else {
            None
        };
        ShellConfig {
            config_dir,
            state_dir,
            app_version: app_version.to_owned(),
        }
    }

    /// A configuration inside `dir`: settings in `dir`, journals in `dir/state`.
    pub fn in_dir(dir: &Path, app_version: &str) -> ShellConfig {
        ShellConfig {
            config_dir: Some(dir.to_path_buf()),
            state_dir: Some(dir.join("state")),
            app_version: app_version.to_owned(),
        }
    }
}

/// What a dispatched action did.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The action ran.
    Done,
    /// The action is disabled now; the key names why (see [`crate::strings`]).
    Refused {
        /// The reason key.
        reason_key: &'static str,
    },
    /// The action ran and failed, or its parameters were wrong.
    Failed {
        /// What went wrong, in English, without file system paths.
        message: String,
    },
}

/// How the open project can be used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessMode {
    /// This session holds the project's lock and may save.
    Edit,
    /// Reading only, for the reason whose text key this is.
    ReadOnly {
        /// The key of the reason text.
        reason_key: &'static str,
    },
}

/// An action that waits for the user's answer to an unsaved-changes dialog.
#[derive(Clone, PartialEq, Debug)]
pub struct Pending {
    /// The action to run again with `discard` set (or after a save).
    pub action: &'static str,
    /// Its parameters.
    pub params: Value,
}

/// A dialog of the shell.
#[derive(Clone, PartialEq, Debug)]
pub enum Dialog {
    /// Help, About.
    About,
    /// Help, Keyboard shortcuts.
    Shortcuts,
    /// Save, Discard or Cancel before an action that would lose changes.
    ConfirmDiscard {
        /// What runs after the answer.
        pending: Pending,
    },
    /// Page, Go to page: a folio typed by the user.
    GotoPage {
        /// The text of the field.
        text: String,
    },
}

/// What the file dialog of the window must ask for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// An existing project directory (Open).
    OpenFolder,
    /// A new, empty directory (Save As).
    NewFolder,
    /// A PDF file name (Export).
    SavePdf,
}

/// A request to the window to ask the user for a path and then dispatch `action` again with it as `param`.
#[derive(Clone, PartialEq, Debug)]
pub struct PromptRequest {
    /// The action to dispatch with the answer.
    pub action: &'static str,
    /// What to ask for.
    pub prompt: Prompt,
    /// The name of the parameter that carries the answer (`dir`, `path`).
    pub param: &'static str,
    /// The other parameters of the action (the `discard` of an Open that already passed the dialog).
    pub params: Value,
}

/// The command palette, while it is open.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PaletteState {
    /// The text of the field.
    pub query: String,
    /// The index of the selected entry among the entries of the query.
    pub selected: usize,
}

/// One `command.run` step that committed, as the command log writes it.
#[derive(Clone, PartialEq, Debug)]
pub struct RecordedCommand {
    /// The command name.
    pub cmd: String,
    /// Its parameters as written in the script.
    pub params: Value,
}

/// The committed `command.run` steps of the project of this run (spec section 6.4).
#[derive(Default)]
struct Record {
    /// The directory the steps belong to; opening another directory starts a new record.
    dir: Option<PathBuf>,
    /// Committed and not undone, in commit order.
    steps: Vec<RecordedCommand>,
    /// Taken back by undo, most recent last; redo restores them.
    undone: Vec<RecordedCommand>,
    /// How many of `steps` a save has put on disk.
    saved: usize,
}

/// A project that was opened and its access decided.
struct Opened {
    session: Session,
    report: OpenReport,
    access: AccessMode,
    banner: Option<String>,
}

struct Project {
    session: Session,
    dir: PathBuf,
    name: String,
    access: AccessMode,
    labels: Vec<String>,
}

/// All state of the application shell.
pub struct ShellModel {
    mode: Mode,
    config: ShellConfig,
    settings: Settings,
    project: Option<Project>,
    page_index: usize,
    banner: Option<String>,
    palette: Option<PaletteState>,
    dialog: Option<Dialog>,
    prompt: Option<PromptRequest>,
    record: Record,
    quit: bool,
    origin_file: String,
}

/// The directory's own name, never its path.
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned())
}

fn failed(key: &str, args: &[&str]) -> Outcome {
    Outcome::Failed {
        message: strings::format(key, args),
    }
}

fn flag(p: &Map<String, Value>, name: &str) -> bool {
    p.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// A string parameter: `Ok(None)` when absent, `Err` when present with another type.
fn string_param<'a>(p: &'a Map<String, Value>, name: &str) -> Result<Option<&'a str>, Outcome> {
    match p.get(name) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(failed("error-bad-param", &[name])),
    }
}

fn read_only_key(reason: &ReadOnlyReason) -> &'static str {
    match reason {
        ReadOnlyReason::Requested => "readonly-requested",
        ReadOnlyReason::LockedBy(_) => "readonly-locked",
        ReadOnlyReason::NotWritable(_) => "readonly-not-writable",
        ReadOnlyReason::Integrity => "readonly-integrity",
        ReadOnlyReason::Format(_) => "readonly-format",
        ReadOnlyReason::LockLost(_) => "readonly-lock-lost",
    }
}

fn open_error_key(e: &OpenError) -> &'static str {
    match e {
        OpenError::Io { .. } => "openerr-io",
        OpenError::NotADirectory(_) => "openerr-not-dir",
        OpenError::Format(_) => "openerr-format",
        OpenError::Migration(_) => "openerr-migration",
        OpenError::SaveInProgress => "openerr-busy",
        OpenError::NoProject(_) => "openerr-no-project",
        OpenError::Model { .. } => "openerr-model",
        OpenError::Takeover(_) => "openerr-takeover",
        OpenError::Exists(_) => "openerr-exists",
        #[allow(unreachable_patterns)]
        _ => "openerr-other",
    }
}

/// The reason key of a save error, from its variant name (`SaveError` is not re-exported by `ew-api`, and
/// its `Display` carries file system paths that a transcript must not).
fn save_error_key(e: &dyn std::fmt::Debug) -> &'static str {
    let text = format!("{e:?}");
    let variant: String = text
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    match variant.as_str() {
        "ReadOnly" => "saveerr-read-only",
        "LockLost" => "saveerr-lock-lost",
        "ExternalChange" => "saveerr-external",
        "Failed" => "saveerr-failed",
        "Incomplete" => "saveerr-incomplete",
        "PendingIncomplete" => "saveerr-pending",
        _ => "saveerr-other",
    }
}

/// Whether dispatching `id` clears the banner: popups, dialogs, help and the describe text leave it standing.
fn clears_banner(id: &str) -> bool {
    !matches!(
        id,
        "ui.describe" | "ui.dismiss" | "tools.palette" | "help.about" | "help.shortcuts" | "view.snapshot"
    )
}

/// Whether `EW_CONFIG_DIR` is set.
fn config_dir_overridden() -> bool {
    settings::env_path("EW_CONFIG_DIR").is_some()
}

impl ShellModel {
    /// A shell in `mode` with the settings of `config` (defaults, and a banner, when the file is bad).
    pub fn new(mode: Mode, config: ShellConfig) -> ShellModel {
        let mut banner = None;
        let settings = match &config.config_dir {
            Some(dir) => {
                let loaded = Settings::load(dir);
                if let Some(name) = &loaded.bad_copy {
                    banner = Some(strings::format("banner-settings-bad", &[name]));
                }
                loaded.settings
            }
            None => Settings::default(),
        };
        ShellModel {
            mode,
            config,
            settings,
            project: None,
            page_index: 0,
            banner,
            palette: None,
            dialog: None,
            prompt: None,
            record: Record::default(),
            quit: false,
            origin_file: "ui-script".to_owned(),
        }
    }

    /// A headless shell with settings in a fresh temporary directory (unit tests of this crate).
    #[cfg(test)]
    pub(crate) fn for_tests() -> ShellModel {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ew-shell-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        ShellModel::new(Mode::Headless, ShellConfig::in_dir(&dir, "0.0.0-test"))
    }

    // -----------------------------------------------------------------------------------------------------------
    // Getters

    /// The mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The configuration the shell was made with.
    pub fn config(&self) -> &ShellConfig {
        &self.config
    }

    /// The settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Whether a project is open.
    pub fn has_project(&self) -> bool {
        self.project.is_some()
    }

    /// The open project's directory name (never its path).
    pub fn project_name(&self) -> Option<&str> {
        self.project.as_ref().map(|p| p.name.as_str())
    }

    /// How the open project can be used.
    pub fn access(&self) -> Option<AccessMode> {
        self.project.as_ref().map(|p| p.access)
    }

    /// Whether the open project can be changed and saved.
    pub fn is_writable(&self) -> bool {
        matches!(self.access(), Some(AccessMode::Edit))
    }

    /// Whether the open project has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.project.as_ref().is_some_and(|p| p.session.is_dirty())
    }

    /// The printed folio labels of the open project's pages, in page order; empty without a project.
    pub fn page_labels(&self) -> &[String] {
        self.project.as_ref().map_or(&[], |p| p.labels.as_slice())
    }

    /// The number of pages.
    pub fn page_count(&self) -> usize {
        self.page_labels().len()
    }

    /// The 0-based index of the current page.
    pub fn page_index(&self) -> usize {
        self.page_index
    }

    /// The folio label of the current page.
    pub fn current_folio(&self) -> Option<&str> {
        self.page_labels().get(self.page_index).map(String::as_str)
    }

    /// The revision of the open project's model, which a rendered page image is tagged with.
    pub fn revision(&self) -> Option<u64> {
        self.project.as_ref().map(|p| p.session.world().rev.0)
    }

    /// Whether the open project has a step to undo.
    pub fn can_undo(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.session.history().cursor() > 0)
    }

    /// Whether the open project has a step to redo.
    pub fn can_redo(&self) -> bool {
        self.project.as_ref().is_some_and(|p| {
            let h = p.session.history();
            h.cursor() < h.entries().len()
        })
    }

    /// The number of recent projects.
    pub fn recent_count(&self) -> usize {
        self.settings.recent_projects.len()
    }

    /// The directory names of the recent projects, most recent first.
    pub fn recent_names(&self) -> Vec<String> {
        self.settings
            .recent_projects
            .iter()
            .map(|p| dir_name(Path::new(p)))
            .collect()
    }

    /// The full paths of the recent projects, most recent first (for a tooltip; never in a transcript).
    pub fn recent_paths(&self) -> &[String] {
        &self.settings.recent_projects
    }

    /// The banner, if one is showing.
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// Shows `text` as the banner (a failure of an action that raised none of its own).
    pub fn show_banner(&mut self, text: &str) {
        self.banner = Some(text.to_owned());
    }

    /// The window title: `<directory name> - ElektraWatt`, with `*` before it while the project is dirty.
    pub fn title(&self) -> String {
        match &self.project {
            None => strings::text("window-title-start").to_owned(),
            Some(p) => {
                let title = strings::format("window-title", &[&p.name]);
                if p.session.is_dirty() {
                    format!("*{title}")
                } else {
                    title
                }
            }
        }
    }

    /// The palette, while it is open.
    pub fn palette(&self) -> Option<&PaletteState> {
        self.palette.as_ref()
    }

    /// The dialog that is open.
    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref()
    }

    /// Whether the application should end (after `app.quit`).
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The path prompt the window must show, taken once.
    pub fn take_prompt(&mut self) -> Option<PromptRequest> {
        self.prompt.take()
    }

    /// The status-bar fields, left to right (spec section 5.3); `exporting` adds the export label of a window
    /// that has started an export and not yet finished it.
    pub fn status_fields(&self, exporting: bool) -> Vec<String> {
        status::fields(self, exporting)
    }

    /// The reason text of a read-only project.
    pub fn read_only_reason(&self) -> Option<&'static str> {
        match self.access() {
            Some(AccessMode::ReadOnly { reason_key }) => Some(strings::text(reason_key)),
            _ => None,
        }
    }

    /// The committed `command.run` steps of this run for `--record`, in commit order.
    pub fn recorded_commands(&self) -> &[RecordedCommand] {
        &self.record.steps
    }

    /// Names the script file for the origin of model commands (`Origin::CmdLog { file }`).
    pub fn set_origin_file(&mut self, name: &str) {
        self.origin_file = name.to_owned();
    }

    /// The sheet display list of one page through the session's memo store (phase 1 of the page view, spec section
    /// 5.3); runs on the thread that owns the model.
    ///
    /// # Errors
    ///
    /// The export job's message when the folio names no page.
    pub fn page_content(&self, folio: &str) -> Result<ExportPages, String> {
        let Some(p) = &self.project else {
            return Err(strings::text("reason-no-project").to_owned());
        };
        p.session
            .export_pages(&ExportOptions {
                pages: PageSelection::Folios(vec![folio.to_owned()]),
                ..ExportOptions::default()
            })
            .map_err(|e| e.to_string())
    }

    /// The names of the registered model commands, sorted.
    pub fn command_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .project
            .as_ref()
            .map(|p| p.session.registry().names().map(str::to_owned).collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    // -----------------------------------------------------------------------------------------------------------
    // The palette (transient widget state)

    /// Sets the text of the palette's field and selects its first entry.
    pub fn set_palette_query(&mut self, query: &str) {
        if let Some(p) = &mut self.palette {
            p.query = query.to_owned();
            p.selected = 0;
        }
    }

    /// Moves the palette's selection by `delta` entries, clamped to the entries of the query.
    pub fn move_palette_selection(&mut self, delta: isize) {
        let count = self.palette_entries().len();
        if let Some(p) = &mut self.palette {
            let last = count.saturating_sub(1);
            p.selected = p.selected.saturating_add_signed(delta).min(last);
        }
    }

    /// Selects the palette entry at `index` (a click), clamped to the entries of the query.
    pub fn select_palette_entry(&mut self, index: usize) {
        let last = self.palette_entries().len().saturating_sub(1);
        if let Some(p) = &mut self.palette {
            p.selected = index.min(last);
        }
    }

    /// The palette entries for the current query: the registry actions and the model commands whose label or ID
    /// contains the query (case-insensitively); enabled entries first, disabled ones last with their reason.
    pub fn palette_entries(&self) -> Vec<PaletteEntry> {
        let query = self
            .palette
            .as_ref()
            .map(|p| p.query.to_lowercase())
            .unwrap_or_default();
        let matches = |label: &str, id: &str| {
            query.is_empty() || label.to_lowercase().contains(&query) || id.to_lowercase().contains(&query)
        };
        let mut entries: Vec<PaletteEntry> = Vec::new();
        for meta in ACTIONS
            .iter()
            .filter(|a| !a.headless_only && a.id != "ui.dismiss")
        {
            let label = strings::text(meta.label_key);
            if !matches(label, meta.id) {
                continue;
            }
            let mut state = (meta.enabled)(self);
            if state.is_ok() && meta.needs_parameters {
                state = Err("reason-parameters-later");
            }
            entries.push(PaletteEntry {
                run: PaletteRun::Action(meta.id),
                label: label.to_owned(),
                id: meta.id.to_owned(),
                shortcut: actions::shortcut_text(meta),
                enabled: state.is_ok(),
                reason: state.err().map(|k| strings::text(k).to_owned()),
            });
        }
        for name in self.command_names() {
            if !matches(&name, &name) {
                continue;
            }
            entries.push(PaletteEntry {
                run: PaletteRun::Command(name.clone()),
                label: name.clone(),
                id: name,
                shortcut: String::new(),
                enabled: false,
                reason: Some(strings::text("reason-parameters-later").to_owned()),
            });
        }
        // Enabled first; the sort is stable, so each group keeps registry order, then command names.
        entries.sort_by_key(|e| !e.enabled);
        entries
    }

    /// Runs the selected palette entry (Enter): closes the palette, then dispatches an enabled action.
    pub fn run_palette_selection(&mut self) -> Outcome {
        let entries = self.palette_entries();
        let selected = self.palette.as_ref().map_or(0, |p| p.selected);
        let Some(entry) = entries.get(selected) else {
            return Outcome::Done;
        };
        if !entry.enabled {
            return match &entry.run {
                PaletteRun::Action(id) => match actions::find(id).map(|m| (m.enabled)(self)) {
                    Some(Err(reason_key)) => Outcome::Refused { reason_key },
                    _ => Outcome::Refused {
                        reason_key: "reason-parameters-later",
                    },
                },
                PaletteRun::Command(_) => Outcome::Refused {
                    reason_key: "reason-parameters-later",
                },
            };
        }
        let PaletteRun::Action(id) = entry.run.clone() else {
            return Outcome::Done;
        };
        self.palette = None;
        self.dispatch(id, &Value::Null)
    }

    /// Sets the text of the go-to-page dialog's field.
    pub fn set_goto_text(&mut self, text: &str) {
        if let Some(Dialog::GotoPage { text: t }) = &mut self.dialog {
            *t = text.to_owned();
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Dispatch

    /// Runs one action of the registry; the one mutating entry point of the shell.
    ///
    /// Menu items, palette entries, shortcuts, dialog results and the headless runner all end here. A disabled action
    /// is [`Outcome::Refused`] with the reason key; a failing one is [`Outcome::Failed`] with an English message
    /// that carries no file system path.
    pub fn dispatch(&mut self, action: &actions::ActionId, params: &Value) -> Outcome {
        let Some(meta) = actions::find(action) else {
            return failed("error-unknown-action", &[action]);
        };
        let empty = Map::new();
        let p = match params {
            Value::Object(m) => m,
            Value::Null => &empty,
            _ => return failed("error-params-object", &[]),
        };
        if clears_banner(meta.id) {
            self.banner = None;
        }
        if let Err(reason_key) = (meta.enabled)(self) {
            return Outcome::Refused { reason_key };
        }
        match meta.id {
            "file.open" => self.act_open(p),
            "file.open_recent" => self.act_open_recent(p),
            "file.save" => self.act_save(),
            "file.save_as" => self.act_save_as(p),
            "file.export_pdf" => self.act_export_pdf(p),
            "file.close" => self.act_close(p),
            "app.quit" => self.act_quit(p),
            "edit.undo" => self.act_undo(),
            "edit.redo" => self.act_redo(),
            "page.next" => self.go_to_index(self.page_index + 1),
            "page.previous" => self.go_to_index(self.page_index.saturating_sub(1)),
            "page.first" => self.go_to_index(0),
            "page.last" => self.go_to_index(self.page_count().saturating_sub(1)),
            "page.goto" => self.act_goto(p),
            "view.fit_page" | "ui.describe" => Outcome::Done,
            "view.ui_scale" => self.act_ui_scale(p),
            "tools.palette" => self.act_palette(p),
            "help.shortcuts" => self.open_dialog(Dialog::Shortcuts),
            "help.about" => self.open_dialog(Dialog::About),
            "command.run" => self.act_command_run(p),
            "view.snapshot" => failed("message-no-snapshot", &[]),
            "ui.dismiss" => self.act_dismiss(),
            other => failed("error-unknown-action", &[other]),
        }
    }

    fn open_dialog(&mut self, dialog: Dialog) -> Outcome {
        self.palette = None;
        self.dialog = Some(dialog);
        Outcome::Done
    }

    /// The unsaved-changes guard of Open, Close and Quit: `Some` when the action must not go on now.
    fn guard_unsaved(&mut self, action: &'static str, p: &Map<String, Value>) -> Option<Outcome> {
        if !self.is_dirty() || flag(p, "discard") {
            return None;
        }
        match self.mode {
            Mode::Headless => Some(failed("error-unsaved", &[])),
            Mode::Gui => {
                self.open_dialog(Dialog::ConfirmDiscard {
                    pending: Pending {
                        action,
                        params: Value::Object(p.clone()),
                    },
                });
                Some(Outcome::Done)
            }
        }
    }

    /// A missing path parameter: a prompt for the window, a failed step for a script.
    fn need_path(
        &mut self,
        action: &'static str,
        prompt: Prompt,
        param: &'static str,
        p: &Map<String, Value>,
    ) -> Outcome {
        match self.mode {
            Mode::Headless => failed("error-missing-param", &[param]),
            Mode::Gui => {
                let mut rest = p.clone();
                rest.remove(param);
                self.prompt = Some(PromptRequest {
                    action,
                    prompt,
                    param,
                    params: Value::Object(rest),
                });
                Outcome::Done
            }
        }
    }

    fn act_open(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.open", p)
        {
            return outcome;
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("file.open", Prompt::OpenFolder, "dir", p),
        };
        self.open_dir(&dir, flag(p, "read_only"))
    }

    fn act_open_recent(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(index) = p.get("index").and_then(Value::as_u64) else {
            return failed("error-missing-param", &["index"]);
        };
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.recent_projects.get(i))
            .cloned()
        else {
            return failed("error-bad-param", &["index"]);
        };
        if self.has_project()
            && let Some(outcome) = self.guard_unsaved("file.open_recent", p)
        {
            return outcome;
        }
        self.open_dir(Path::new(&path), false)
    }

    /// Closes the open project (if any) and opens `dir`: a failure leaves the start page with a banner.
    fn open_dir(&mut self, dir: &Path, read_only: bool) -> Outcome {
        self.close_project(false);
        let name = dir_name(dir);
        let path_text = dir.to_string_lossy().into_owned();
        let version = self.config.app_version.clone();
        let opened = if read_only {
            Session::open_read_only(dir, &version)
        } else {
            self.open_for_editing(dir)
        };
        let opened = opened.and_then(|(session, report)| self.settle_open(dir, session, report, read_only));
        let Opened {
            session,
            report,
            access,
            banner,
        } = match opened {
            Ok(o) => o,
            Err(e) => {
                let message = strings::format("error-open", &[&name, strings::text(open_error_key(&e))]);
                if self.settings.recent_projects.contains(&path_text) {
                    self.settings.remove_recent(&path_text);
                    self.save_settings();
                    self.banner = Some(strings::format("banner-recent-removed", &[&name]));
                } else {
                    self.banner = Some(message.clone());
                }
                return Outcome::Failed { message };
            }
        };
        let findings = ew_api::integrity_items(&report)
            .iter()
            .filter(|i| i.severity != ew_api::checks::Severity::Info)
            .count();
        self.banner = match (banner, findings) {
            (b, 0) => b,
            (None, n) => Some(strings::format("banner-findings", &[&n.to_string()])),
            (Some(b), n) => Some(format!(
                "{b} {}",
                strings::format("banner-findings", &[&n.to_string()])
            )),
        };
        let labels = session.page_labels();
        if self.record.dir.as_deref() != Some(dir) {
            self.record = Record {
                dir: Some(dir.to_path_buf()),
                ..Record::default()
            };
        }
        self.project = Some(Project {
            session,
            dir: dir.to_path_buf(),
            name,
            access,
            labels,
        });
        self.page_index = 0;
        self.settings.push_recent(&path_text);
        self.save_settings();
        Outcome::Done
    }

    fn open_for_editing(&self, dir: &Path) -> Result<(Session, OpenReport), OpenError> {
        let mut store = StoreOptions::new(&self.config.app_version);
        if let Some(state) = &self.config.state_dir {
            store.state_dir = Some(state.clone());
        }
        let options = SessionOptions {
            origin: Origin::Gui,
            ..SessionOptions::default()
        };
        Session::open(dir, store, options, Recovery::Keep)
    }

    /// Decides the access of a freshly opened session: a leftover journal re-opens it read-only.
    fn settle_open(
        &self,
        dir: &Path,
        session: Session,
        report: OpenReport,
        read_only: bool,
    ) -> Result<Opened, OpenError> {
        if !read_only && report.recovery.is_some() {
            // The editing session is closed again (its own journal goes, the leftover one stays) and the project
            // re-opens through `open_read_only`; recovery itself is FT-313.
            session.close();
            let (again, again_report) = Session::open_read_only(dir, &self.config.app_version)?;
            return Ok(Opened {
                session: again,
                report: again_report,
                access: AccessMode::ReadOnly {
                    reason_key: "readonly-recovery",
                },
                banner: Some(strings::text("banner-recovery").to_owned()),
            });
        }
        Ok(match &report.access {
            Access::Edit => Opened {
                session,
                report,
                access: AccessMode::Edit,
                banner: None,
            },
            Access::ReadOnly(reason) => {
                let key = read_only_key(reason);
                let banner = (!read_only).then(|| strings::format("banner-read-only", &[strings::text(key)]));
                Opened {
                    session,
                    report,
                    access: AccessMode::ReadOnly { reason_key: key },
                    banner,
                }
            }
        })
    }

    fn close_project(&mut self, discard: bool) {
        if let Some(p) = self.project.take() {
            if discard {
                self.record.steps.truncate(self.record.saved);
                self.record.undone.clear();
            }
            p.session.close();
        }
        self.page_index = 0;
    }

    /// Ends the session cleanly without saving: for the end of a run, so no journal stays behind.
    pub fn shutdown(&mut self) {
        self.close_project(true);
    }

    fn save_settings(&mut self) {
        if let Some(dir) = &self.config.config_dir
            && self.settings.save(dir).is_err()
        {
            self.banner = Some(strings::text("banner-settings-write").to_owned());
        }
    }

    fn refresh(&mut self) {
        let Some(p) = &mut self.project else { return };
        let folio = p.labels.get(self.page_index).cloned();
        p.labels = p.session.page_labels();
        self.page_index = match folio.and_then(|f| p.labels.iter().position(|l| *l == f)) {
            Some(i) => i,
            None => self.page_index.min(p.labels.len().saturating_sub(1)),
        };
    }

    fn act_save(&mut self) -> Outcome {
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        match p.session.save() {
            Ok(_) => {
                self.record.saved = self.record.steps.len();
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format("error-save", &[strings::text(save_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    fn act_save_as(&mut self, p: &Map<String, Value>) -> Outcome {
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("file.save_as", Prompt::NewFolder, "dir", p),
        };
        let mut store = StoreOptions::new(&self.config.app_version);
        if let Some(state) = &self.config.state_dir {
            store.state_dir = Some(state.clone());
        }
        let Some(project) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        match project.session.create(&dir, store) {
            Ok(()) => {
                project.dir = dir.clone();
                project.name = dir_name(&dir);
                project.access = AccessMode::Edit;
                self.record.dir = Some(dir.clone());
                self.record.saved = self.record.steps.len();
                self.settings.push_recent(&dir.to_string_lossy());
                self.save_settings();
                Outcome::Done
            }
            Err(e) => {
                let message = strings::format("error-save-as", &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    fn act_export_pdf(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => PathBuf::from(path),
            Ok(None) => return self.need_path("file.export_pdf", Prompt::SavePdf, "path", p),
        };
        let Some(project) = &self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let export = match project.session.export_pdf(&ExportOptions::default()) {
            Ok(export) => export,
            Err(e) => {
                let message = strings::format("error-export", &[&e.to_string()]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        if std::fs::write(&path, &export.bytes).is_err() {
            let message = strings::format("error-export", &[strings::text("error-write")]);
            self.banner = Some(message.clone());
            return Outcome::Failed { message };
        }
        Outcome::Done
    }

    fn act_close(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(outcome) = self.guard_unsaved("file.close", p) {
            return outcome;
        }
        self.close_project(flag(p, "discard"));
        Outcome::Done
    }

    fn act_quit(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(outcome) = self.guard_unsaved("app.quit", p) {
            return outcome;
        }
        self.close_project(true);
        self.quit = true;
        Outcome::Done
    }

    fn act_undo(&mut self) -> Outcome {
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let before = p.session.history().cursor();
        match p.session.undo() {
            Ok(_) => {
                if p.session.history().cursor() < before
                    && let Some(step) = self.record.steps.pop()
                {
                    self.record.undone.push(step);
                    self.record.saved = self.record.saved.min(self.record.steps.len());
                }
                self.refresh();
                Outcome::Done
            }
            Err(e) => failed("error-command", &[&e.to_string()]),
        }
    }

    fn act_redo(&mut self) -> Outcome {
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let before = p.session.history().cursor();
        match p.session.redo() {
            Ok(_) => {
                if p.session.history().cursor() > before
                    && let Some(step) = self.record.undone.pop()
                {
                    self.record.steps.push(step);
                }
                self.refresh();
                Outcome::Done
            }
            Err(e) => failed("error-command", &[&e.to_string()]),
        }
    }

    fn go_to_index(&mut self, index: usize) -> Outcome {
        self.page_index = index.min(self.page_count().saturating_sub(1));
        Outcome::Done
    }

    fn act_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(folio)) => folio.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed("error-missing-param", &["folio"]),
                    Mode::Gui => self.open_dialog(Dialog::GotoPage { text: String::new() }),
                };
            }
        };
        match self.page_labels().iter().position(|l| *l == folio) {
            Some(i) => {
                self.dialog = None;
                self.page_index = i;
                Outcome::Done
            }
            None => failed("error-no-folio", &[&folio]),
        }
    }

    fn act_ui_scale(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(scale) = p.get("scale").and_then(Value::as_f64) else {
            return failed("error-missing-param", &["scale"]);
        };
        let scale = scale as f32;
        if scale_index(scale).is_none() {
            return failed("error-bad-param", &["scale"]);
        }
        self.settings.ui_scale = UI_SCALES[scale_index(scale).unwrap_or(0)];
        self.save_settings();
        Outcome::Done
    }

    fn act_palette(&mut self, p: &Map<String, Value>) -> Outcome {
        let query = match string_param(p, "query") {
            Err(e) => return e,
            Ok(q) => q,
        };
        if self.palette.is_some() && query.is_none() {
            self.palette = None;
        } else {
            self.dialog = None;
            self.palette = Some(PaletteState {
                query: query.unwrap_or("").to_owned(),
                selected: 0,
            });
        }
        Outcome::Done
    }

    fn act_dismiss(&mut self) -> Outcome {
        if self.palette.take().is_some() || self.dialog.take().is_some() {
            return Outcome::Done;
        }
        self.banner = None;
        Outcome::Done
    }

    /// Answers the unsaved-changes dialog with Save (save, then the pending action), Discard (the pending action with
    /// `discard`) or Cancel (nothing).
    pub fn answer_confirm(&mut self, answer: ConfirmAnswer) -> Outcome {
        let Some(Dialog::ConfirmDiscard { pending }) = self.dialog.clone() else {
            return Outcome::Done;
        };
        self.dialog = None;
        let mut params = pending.params.as_object().cloned().unwrap_or_default();
        match answer {
            ConfirmAnswer::Cancel => Outcome::Done,
            ConfirmAnswer::Discard => {
                params.insert("discard".to_owned(), Value::Bool(true));
                self.dispatch(pending.action, &Value::Object(params))
            }
            ConfirmAnswer::Save => match self.dispatch("file.save", &Value::Null) {
                Outcome::Done => self.dispatch(pending.action, &Value::Object(params)),
                other => other,
            },
        }
    }

    fn act_command_run(&mut self, p: &Map<String, Value>) -> Outcome {
        let cmd = match string_param(p, "cmd") {
            Err(e) => return e,
            Ok(Some(cmd)) => cmd.to_owned(),
            Ok(None) => return failed("error-missing-param", &["cmd"]),
        };
        let params = match p.get("params") {
            None | Some(Value::Null) => json!({}),
            Some(v @ Value::Object(_)) => v.clone(),
            Some(_) => return failed("error-bad-param", &["params"]),
        };
        // One command-log line through the session's own replay: reference resolution, preview and commit for bulk
        // commands, one undo step, origin CmdLog (spec section 6.3).
        let text = format!(
            "{}\n{}\n",
            json!({"$schema": ew_api::CMDLOG_SCHEMA, "label": self.origin_file}),
            json!({"cmd": cmd, "params": params})
        );
        let log = match CmdLog::parse(&text) {
            Ok(log) => log,
            Err(e) => return failed("error-command", &[&e.to_string()]),
        };
        let file = self.origin_file.clone();
        let Some(project) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        let before = project.session.history().cursor();
        let result = project.session.run_script(&log, &file);
        let committed = project.session.history().cursor() > before;
        let outcome = match result {
            Ok(_) => Outcome::Done,
            Err(e) => failed("error-command", &[&e.to_string()]),
        };
        if committed && outcome == Outcome::Done {
            self.record.undone.clear();
            self.record.steps.push(RecordedCommand { cmd, params });
        }
        self.refresh();
        outcome
    }
}

/// The answers of the unsaved-changes dialog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmAnswer {
    /// Save, then go on.
    Save,
    /// Go on without saving.
    Discard,
    /// Stay.
    Cancel,
}
