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

use std::cell::Cell;
use std::path::{Path, PathBuf};

use ew_api::output::{ExportOptions, ExportPages, PageRow, PageSelection};
use ew_api::{
    Access, CmdLog, OpenError, OpenReport, Origin, ReadOnlyReason, Recovery, SaveError, Session,
    SessionOptions, StoreOptions,
};
use serde_json::{Map, Value, json};

use crate::actions::{self, ACTIONS, PaletteEntry, PaletteRun};
use crate::area::CanvasArea;
use crate::canvas::{CanvasBackend, HEADLESS_PANE};
use crate::first_run::FirstRun;
use crate::layout::Layout;
use crate::navigator::PageNavigator;
use crate::settings::{self, Settings, UI_SCALES, scale_index};
use crate::status;
use crate::strings;
use crate::wizard::Wizard;

mod canvas_actions;
mod editing;
// WP-S2.05 part 2
mod find;
mod navigation;
mod onboarding;
mod projects;
mod properties;
mod tools;
// WP-S2.04 part 2: the wiring tools' host duties.
mod wiring;
mod workspaces;

pub use navigation::{Focus, RECENT_PAGES, SessionCalls};
pub use tools::SnapIndicator;

/// Who drives the shell: the window or the headless runner. They share `dispatch`; the modes differ only where a
/// person can be asked (a dialog) and a script cannot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// The window: a missing parameter becomes a prompt, unsaved changes become a dialog.
    Gui,
    /// No window: a missing parameter fails the step, unsaved changes fail it unless `discard` is `true`.
    Headless,
}

/// Where the application's read-only resources are (the installation, or the source tree of a development build).
/// The application binary finds them; a missing one disables only the feature that needs it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Resources {
    /// The sample project directory (`J-E-2416.ew`, CC0, NFR-DOC-04): Open sample project, the tutorials, the
    /// wizard's live preview.
    pub sample_dir: Option<PathBuf>,
    /// The starter content (`fndefs`, `forms`, `symbols`) that the QElectroTech import references.
    pub content_dir: Option<PathBuf>,
    /// The built user guide (`index.html` and its pages), which F1 opens in the system browser.
    pub docs_dir: Option<PathBuf>,
    /// The generated third-party notices (`THIRD-PARTY-NOTICES.txt`), shown in the notices dialog.
    pub notices_file: Option<PathBuf>,
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
    /// The read-only resources.
    pub resources: Resources,
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
            resources: Resources::default(),
        }
    }

    /// The configuration of a headless run: `EW_CONFIG_DIR` when set, else no settings directory at all, so
    /// that a script neither reads nor writes the user's own settings and recent projects (its journals still
    /// go to the store's default state directory, where recovery looks for them).
    pub fn isolated_from_env(app_version: &str) -> ShellConfig {
        if config_dir_overridden() {
            ShellConfig::from_env(app_version)
        } else {
            ShellConfig {
                config_dir: None,
                state_dir: None,
                app_version: app_version.to_owned(),
                resources: Resources::default(),
            }
        }
    }

    /// A configuration inside `dir`: settings in `dir`, journals in `dir/state`.
    pub fn in_dir(dir: &Path, app_version: &str) -> ShellConfig {
        ShellConfig {
            config_dir: Some(dir.to_path_buf()),
            state_dir: Some(dir.join("state")),
            app_version: app_version.to_owned(),
            resources: Resources::default(),
        }
    }

    /// The same configuration with these resources.
    #[must_use]
    pub fn with_resources(mut self, resources: Resources) -> ShellConfig {
        self.resources = resources;
        self
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
    /// Page, Go to page or cell: a folio or a cell typed by the user.
    GotoPage {
        /// The text of the field.
        text: String,
    },
    /// Help, Third-party notices.
    Notices,
    /// Discard the journal of an earlier session that ended with unsaved changes (FT-313's first part): the
    /// changes in it are lost.
    ConfirmJournalDiscard,
    /// Apply the check scheme to the finished basic project (FT-336): `project.checks.set` is `HumanOnly`, so a
    /// person confirms it, with the scheme named and the note that it decides check outcomes.
    ConfirmCheckScheme {
        /// The scheme to apply (`release_scheme`).
        scheme: String,
    },
    /// View, Workspace, Save layout as: a name typed by the user.
    SaveWorkspace {
        /// The text of the field.
        text: String,
    },
}

/// What the file dialog of the window must ask for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prompt {
    /// An existing project directory (Open).
    OpenFolder,
    /// A new, empty directory (Save As, New project, the wizard's basic project).
    NewFolder,
    /// A PDF file name (Export).
    SavePdf,
    /// An existing QElectroTech project file (Import).
    OpenQet,
}

/// A request to the window to open a page of the user guide in the system browser (F1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocRequest {
    /// The page and heading below the book's output directory (`how-to/application-window.html#start-page`).
    pub target: String,
    /// The `file://` URL, when the built book is installed; `None` when it is not.
    pub url: Option<String>,
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
    /// `steps` as they were at the last save: what the project on disk holds. A close or a discard puts the steps
    /// back to this, so what a reopen finds is what the record says, whatever was undone past a save.
    saved_steps: Vec<RecordedCommand>,
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
    /// The number of this project session in the shell's lifetime (the canvas's session key: revisions restart
    /// with every session).
    epoch: u64,
    dir: PathBuf,
    name: String,
    access: AccessMode,
    labels: Vec<String>,
    /// The rows of the derived page order (`Session::page_rows`), once per revision.
    rows: Vec<PageRow>,
}

/// All state of the application shell.
pub struct ShellModel {
    mode: Mode,
    config: ShellConfig,
    settings: Settings,
    project: Option<Project>,
    banner: Option<String>,
    palette: Option<PaletteState>,
    dialog: Option<Dialog>,
    prompt: Option<PromptRequest>,
    record: Record,
    quit: bool,
    origin_file: String,
    first_run: Option<FirstRun>,
    wizard: Option<Wizard>,
    /// The dock layout of the project view (FT-309) and the workspace it belongs to.
    layout: Layout,
    workspace: String,
    /// Bumped when the shell replaces the layout (a workspace was loaded or reset): the frame rebuilds its tree.
    layout_version: u64,
    /// The time (egui seconds) of the last layout change that is not on disk yet.
    layout_changed_at: Option<f64>,
    /// The documentation page F1 asked for and the window has not opened yet.
    doc_request: Option<DocRequest>,
    /// The target of the last documentation request, kept for `ui.describe`.
    last_help: Option<String>,
    /// The open pages as tabs in one or two groups (WP-S2.03): each tab's canvas, view memory and history.
    area: Option<CanvasArea>,
    /// The page navigator (WP-S2.03 §6.1).
    navigator: PageNavigator,
    /// Where the keyboard focus is.
    focus: Option<Focus>,
    /// The page content of the current revision by folio (WP-S2.03 §6.2).
    cache: navigation::PageCache,
    /// The viewer's `Session` reads, counted.
    calls: Cell<SessionCalls>,
    /// The canvas pane of a new project's first group in logical points and its pixels per point.
    canvas_pane: (f64, f64),
    canvas_ppp: f64,
    /// Whether the page grid is drawn.
    grid_on: bool,
    /// Whether the frame-statistics overlay is shown.
    frame_stats: bool,
    /// Where the canvas draws (set by the host).
    canvas_backend: CanvasBackend,
    /// The (session epoch, revision, folio) whose content could not be built, and why.
    canvas_error: Option<(u64, u64, String, String)>,
    /// How many sheet display lists the shell has built (engine-statistics tests).
    page_content_calls: Cell<u64>,
    /// The project sessions opened so far (the source of [`Project`]'s epoch).
    epochs: u64,
    /// The canvas tool host (WP-S2.03 part 2): the active tool, its overlay, the preview, the selection.
    tools: crate::tools::ToolHost,
    /// The property grid (WP-S2.05 part 1): the sheet, the row cursor, the edit, the card and the toast.
    props: crate::properties::PropertiesState,
    /// The find pane (WP-S2.05 part 2): the query, the results, the cursor and the note.
    find: crate::find::FindState,
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

/// The reason key of a save error (its `Display` carries file system paths that a transcript must not).
fn save_error_key(e: &SaveError) -> &'static str {
    match e {
        SaveError::ReadOnly(_) => "saveerr-read-only",
        SaveError::LockLost(_) => "saveerr-lock-lost",
        SaveError::ExternalChange => "saveerr-external",
        SaveError::Failed { .. } => "saveerr-failed",
        SaveError::Incomplete { .. } => "saveerr-incomplete",
        SaveError::PendingIncomplete { .. } => "saveerr-pending",
        _ => "saveerr-other",
    }
}

/// Whether dispatching `id` clears the banner: popups, dialogs, help and the describe text leave it standing.
fn clears_banner(id: &str) -> bool {
    !matches!(
        id,
        "ui.describe"
            | "ui.dismiss"
            | "tools.palette"
            | "help.about"
            | "help.shortcuts"
            | "help.notices"
            | "help.documentation"
            | "file.discard_journal"
            | "view.snapshot"
            // The canvas's view actions leave a banner standing: moving the pointer or resizing the window must not
            // dismiss what the banner says (WP-S2.02 lead ruling).
            | "view.fit_page"
            | "view.zoom_in"
            | "view.zoom_out"
            | "view.zoom_actual"
            | "view.pan_left"
            | "view.pan_right"
            | "view.pan_up"
            | "view.pan_down"
            | "view.grid"
            | "view.frame_stats"
            | "view.zoom_to"
            | "view.pan"
            | "view.canvas_resize"
            | "canvas.pointer"
            | "canvas.snapshot"
            // The viewer's navigation leaves a banner standing (WP-S2.03 §6.7); nav.goto, page.goto and tab.open,
            // which can fail with a message, clear it.
            | "tab.new"
            | "tab.close"
            | "tab.next"
            | "tab.previous"
            | "tab.focus_other"
            | "tab.activate"
            | "tab.move"
            | "view.side_by_side"
            | "nav.back"
            | "nav.forward"
            | "nav.follow"
            | "nav.next_link"
            | "nav.previous_link"
            | "cursor.left"
            | "cursor.right"
            | "cursor.up"
            | "cursor.down"
            | "cursor.cell_left"
            | "cursor.cell_right"
            | "cursor.cell_up"
            | "cursor.cell_down"
            | "cursor.set"
            | "pages.mode"
            | "pages.sort"
            | "pages.filter"
            | "pages.toggle"
            | "pages.row"
            | "pages.open"
            | "ui.focus"
            | "ui.focus_next"
            | "ui.focus_previous"
            // The tool framework's input leaves a banner standing too (WP-S2.03 part 2): a pointer, a press, a rest or
            // a key of the canvas must not dismiss what the banner says (a failed commit, for one).
            | "canvas.press"
            | "canvas.release"
            | "canvas.dwell"
            | "canvas.activate"
            | "canvas.preview_snapshot"
            | "tool.select"
            | "tool.measure"
            | "tool.back"
            | "cp.next"
            | "cp.previous"
            | "cp.next_unconnected"
            // The place-and-arrange actions (WP-S2.04 §7.6) leave it standing too.
            | "tool.place"
            | "tool.place_part"
            | "place.pick"
            | "picker.filter"
            | "picker.row"
            | "picker.choose"
            | "strip.new"
            | "tool.rotate_cw"
            | "tool.rotate_ccw"
            | "tool.mirror_x"
            | "tool.mirror_y"
            | "tool.next_variant"
            | "tool.next_rep_type"
            | "tool.move"
            | "tool.finish"
            | "text.input"
            | "edit.delete_selection"
            | "rep.exchange"
            // WP-S2.04 part 2 (wiring, §8.7) leaves it standing too. Begin.
            | "tool.wire"
            | "canvas.activate_alt"
            | "tool.next_proposal"
            | "tool.connect_on_move"
            | "tool.route_edit"
            | "junction.open"
            | "junction.member"
            | "junction.style"
            | "junction.wiring"
            | "project.crossings"
            // WP-S2.04 part 2. End.
            // The property grid's actions (WP-S2.05 §6.10): a failed edit shows under its field, not in the banner.
            | "props.show"
            | "props.focus"
            | "props.project"
            | "select.set"
            | "props.row"
            | "props.category"
            | "props.edit"
            | "props.input"
            | "props.commit"
            | "props.cancel"
            | "props.set"
            | "props.choose"
            | "props.row_menu"
            | "props.action"
            | "props.list"
            | "props.mltext"
            | "preview.apply"
            | "preview.cancel"
            | "toast.undo"
            | "toast.dismiss"
            // The find pane's actions (WP-S2.05 §7.6): an error shows in the pane.
            | "find.open"
            | "find.open_replace"
            | "find.close"
            | "find.text"
            | "find.replacement"
            | "find.option"
            | "find.fields"
            | "find.folios"
            | "find.run"
            | "find.row"
            | "find.jump"
            | "find.replace_all"
    )
}

/// Whether dispatching `id` leaves the link hover preview standing: the rest that shows it, the describe text, the
/// snapshots and Escape (which closes it as the topmost popup); every other action (a pointer move, a cursor move, a
/// view action) clears it (WP-S2.03 §7.8).
fn keeps_link_preview(id: &str) -> bool {
    matches!(
        id,
        "canvas.dwell"
            | "ui.describe"
            | "ui.dismiss"
            | "canvas.preview_snapshot"
            | "canvas.snapshot"
            | "view.snapshot"
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
        let workspace = settings.workspace.clone();
        let mut model = ShellModel {
            mode,
            config,
            settings,
            project: None,
            banner,
            palette: None,
            dialog: None,
            prompt: None,
            record: Record::default(),
            quit: false,
            origin_file: "ui-script".to_owned(),
            first_run: None,
            wizard: None,
            layout: Layout::schematic(),
            workspace,
            layout_version: 0,
            layout_changed_at: None,
            doc_request: None,
            last_help: None,
            area: None,
            navigator: PageNavigator::new(),
            focus: None,
            cache: navigation::PageCache::default(),
            calls: Cell::new(SessionCalls::default()),
            canvas_pane: HEADLESS_PANE,
            canvas_ppp: 1.0,
            grid_on: true,
            frame_stats: false,
            canvas_backend: CanvasBackend::Cpu,
            canvas_error: None,
            page_content_calls: Cell::new(0),
            epochs: 0,
            tools: crate::tools::ToolHost::new(),
            props: crate::properties::PropertiesState::default(),
            find: crate::find::FindState::default(),
        };
        model.load_active_workspace();
        model
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

    /// The 0-based index of the current page: the focused group's active tab's page in page order.
    pub fn page_index(&self) -> usize {
        self.current_page().and_then(|p| self.row_index(p)).unwrap_or(0)
    }

    /// The folio label of the current page.
    pub fn current_folio(&self) -> Option<&str> {
        self.page_labels().get(self.page_index()).map(String::as_str)
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

    /// The company-standard wizard, while it is open.
    pub fn wizard(&self) -> Option<&Wizard> {
        self.wizard.as_ref()
    }

    /// The first-run flow, while it is open.
    pub fn first_run(&self) -> Option<&FirstRun> {
        self.first_run.as_ref()
    }

    /// The dock layout of the project view.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// A number that changes when the shell replaces the layout (a workspace was loaded or reset): the frame
    /// rebuilds its dock tree from [`ShellModel::layout`] then.
    pub fn layout_version(&self) -> u64 {
        self.layout_version
    }

    /// The open project is read-only because an earlier session left a journal: it can be discarded.
    pub fn can_discard_journal(&self) -> bool {
        matches!(
            self.access(),
            Some(AccessMode::ReadOnly {
                reason_key: "readonly-recovery"
            })
        )
    }

    /// The documentation page F1 asked for, taken once by the window.
    pub fn take_doc_request(&mut self) -> Option<DocRequest> {
        self.doc_request.take()
    }

    /// The target of the last documentation request (`how-to/application-window.html#start-page`).
    pub fn last_help(&self) -> Option<&str> {
        self.last_help.as_deref()
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
        self.page_content_calls.set(self.page_content_calls.get() + 1);
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
        for meta in ACTIONS.iter().filter(|a| !a.headless_only && !a.hidden) {
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
        if !keeps_link_preview(meta.id) {
            self.tools.link_preview = None;
        }
        // The toast stays until it is dismissed or replaced by the next committing action (WP-S2.05 §6.6).
        if meta.kind == actions::ActionKind::Command && !meta.id.starts_with("toast.") {
            self.props.toast = None;
        }
        let outcome = self.run_action(meta.id, p);
        // The canvases follow the pages and the revision the action left (WP-S2.02 section 6.1, WP-S2.03 §6.2), and a
        // busy tool is escaped when its canvas changed (WP-S2.03 §7.2); the property grid fetches its sheet when its
        // key changed (WP-S2.05 §9 item 4).
        self.sync_views();
        self.sync_tool_context();
        self.after_commit();
        // WP-S2.04 part 2: the junction editor follows the revision and the page.
        self.sync_junction_editor();
        self.sync_properties();
        self.sync_find();
        outcome
    }

    /// The body of [`ShellModel::dispatch`] for an enabled action.
    fn run_action(&mut self, id: &'static str, p: &Map<String, Value>) -> Outcome {
        match id {
            "file.new" => self.act_new(p),
            "file.open_sample" => self.act_open_sample(p),
            "file.import_qet" => self.act_import_qet(p),
            "file.discard_journal" => self.act_discard_journal(p),
            "view.workspace_load" => self.act_workspace_load(p),
            "view.workspace_save" => self.act_workspace_save(p),
            "view.workspace_reset" => self.act_workspace_reset(),
            "settings.set" => self.act_settings_set(p),
            "help.documentation" => self.act_documentation(p),
            "help.tutorials" => self.act_tutorials(p),
            "help.first_run" => self.act_first_run_open(p),
            "help.notices" => self.open_dialog(Dialog::Notices),
            "wizard.apply_check_scheme" => self.act_wizard_apply_check_scheme(p),
            "first_run.set" => self.act_first_run_set(p),
            "first_run.next" => self.act_first_run_step(1),
            "first_run.back" => self.act_first_run_step(-1),
            "first_run.finish" => self.act_first_run_finish(p),
            "tools.company_standard" => self.act_wizard_open(),
            "wizard.set" => self.act_wizard_set(p),
            "wizard.next" => self.act_wizard_step(1),
            "wizard.back" => self.act_wizard_step(-1),
            "wizard.goto" => self.act_wizard_goto(p),
            "wizard.reset" => self.act_wizard_reset(),
            "wizard.cancel" => self.act_wizard_cancel(),
            "wizard.finish" => self.act_wizard_finish(p),
            "file.open" => self.act_open(p),
            "file.open_recent" => self.act_open_recent(p),
            "file.save" => self.act_save(),
            "file.save_as" => self.act_save_as(p),
            "file.export_pdf" => self.act_export_pdf(p),
            "file.close" => self.act_close(p),
            "app.quit" => self.act_quit(p),
            "edit.undo" => self.act_undo(),
            "edit.redo" => self.act_redo(),
            "page.next" => self.walk(self.page_index() + 1),
            "page.previous" => self.walk(self.page_index().saturating_sub(1)),
            "page.first" => self.walk(0),
            "page.last" => self.walk(self.page_count().saturating_sub(1)),
            "page.goto" => self.act_page_goto(p),
            "nav.goto" => self.act_nav_goto(p),
            "nav.back" => self.act_history(true),
            "nav.forward" => self.act_history(false),
            "nav.follow" => self.act_follow(p),
            "nav.next_link" => self.act_link_step(p, true),
            "nav.previous_link" => self.act_link_step(p, false),
            "tab.new" => self.act_tab_new(),
            "tab.close" => self.act_tab_close(),
            "tab.next" => self.act_tab_cycle(1),
            "tab.previous" => self.act_tab_cycle(-1),
            "tab.focus_other" => self.act_focus_other(),
            "tab.activate" => self.act_tab_activate(p),
            "tab.move" => self.act_tab_move(p),
            "tab.open" => self.act_tab_open(p),
            "view.side_by_side" => self.act_side_by_side(),
            "cursor.left" => self.act_cursor(p, -1, 0, false),
            "cursor.right" => self.act_cursor(p, 1, 0, false),
            "cursor.up" => self.act_cursor(p, 0, -1, false),
            "cursor.down" => self.act_cursor(p, 0, 1, false),
            "cursor.cell_left" => self.act_cursor(p, -1, 0, true),
            "cursor.cell_right" => self.act_cursor(p, 1, 0, true),
            "cursor.cell_up" => self.act_cursor(p, 0, -1, true),
            "cursor.cell_down" => self.act_cursor(p, 0, 1, true),
            "cursor.set" => self.act_cursor_set(p),
            "pages.mode" => self.act_pages_mode(p),
            "pages.sort" => self.act_pages_sort(p),
            "pages.filter" => self.act_pages_filter(p),
            "pages.toggle" => self.act_pages_toggle(p),
            "pages.row" => self.act_pages_row(p),
            "pages.open" => self.act_pages_open(p),
            "ui.focus" => self.act_focus(p),
            "ui.focus_next" => self.act_focus_cycle(1),
            "ui.focus_previous" => self.act_focus_cycle(-1),
            "view.fit_page" => self.act_fit_page(p),
            "view.zoom_in" => self.act_zoom_in(p),
            "view.zoom_out" => self.act_zoom_out(p),
            "view.zoom_actual" => self.act_zoom_actual(p),
            "view.pan_left" => self.act_pan_step(p, -1.0, 0.0),
            "view.pan_right" => self.act_pan_step(p, 1.0, 0.0),
            "view.pan_up" => self.act_pan_step(p, 0.0, -1.0),
            "view.pan_down" => self.act_pan_step(p, 0.0, 1.0),
            "view.grid" => self.act_grid(),
            "view.frame_stats" => self.act_frame_stats(),
            "view.cpu_canvas" => self.act_cpu_canvas(),
            "view.zoom_to" => self.act_zoom_to(p),
            "view.pan" => self.act_pan(p),
            "view.canvas_resize" => self.act_canvas_resize(p),
            "canvas.pointer" => self.act_pointer(p),
            "canvas.snapshot" | "canvas.preview_snapshot" => failed("message-no-snapshot", &[]),
            "canvas.press" => self.act_press(p, false),
            "canvas.release" => self.act_press(p, true),
            "canvas.dwell" => self.act_dwell(p),
            "canvas.activate" => self.act_activate_with(p),
            "tool.back" => self.act_tool_back(),
            "tool.select" => self.act_tool("select"),
            "tool.measure" => self.act_tool("measure"),
            "cp.next" => self.act_cp_step(p, tools::CpStep::Next),
            "cp.previous" => self.act_cp_step(p, tools::CpStep::Previous),
            "cp.next_unconnected" => self.act_cp_step(p, tools::CpStep::NextUnconnected),
            "tool.place" => self.act_open_picker(crate::tools::PickerMode::Place),
            "tool.place_part" => self.act_open_picker(crate::tools::PickerMode::Parts),
            "place.pick" => self.act_place_pick(p),
            "picker.filter" => self.act_picker_filter(p),
            "picker.row" => self.act_picker_row(p),
            "picker.choose" => self.act_picker_choose(p),
            "strip.new" => self.act_strip_new(p),
            "tool.rotate_cw" => self.act_tool_key(crate::tools::ToolKey::RotateCw),
            "tool.rotate_ccw" => self.act_tool_key(crate::tools::ToolKey::RotateCcw),
            "tool.mirror_x" => self.act_tool_key(crate::tools::ToolKey::MirrorX),
            "tool.mirror_y" => self.act_tool_key(crate::tools::ToolKey::MirrorY),
            "tool.next_variant" => self.act_tool_key(crate::tools::ToolKey::NextVariant),
            "tool.next_rep_type" => self.act_tool_key(crate::tools::ToolKey::NextRepType),
            "tool.move" => self.act_tool_move(),
            "tool.finish" => self.send_tool(&crate::tools::CanvasEvent::Key(crate::tools::ToolKey::Finish)),
            "text.input" => match string_param(p, "text") {
                Ok(Some(t)) => self.send_tool(&crate::tools::CanvasEvent::Text(t.to_owned())),
                Ok(None) => failed("error-missing-param", &["text"]),
                Err(e) => e,
            },
            "edit.delete_selection" => self.act_delete_selection(),
            "rep.exchange" => self.act_rep_exchange(p),
            // WP-S2.04 part 2 (wiring, §8.7). Begin.
            "tool.wire" => self.act_tool_wire(),
            "canvas.activate_alt" => {
                let mut alt = p.clone();
                alt.insert("alt".into(), Value::Bool(true));
                self.act_activate_with(&alt)
            }
            "tool.next_proposal" => self.send_proposal_key(),
            "tool.connect_on_move" => {
                // Refused in this package (§8.3); the status bar says why, the window has no other trace of a key.
                self.tools.notice = Some("reason-connect-on-place");
                Outcome::Refused {
                    reason_key: "reason-connect-on-place",
                }
            }
            "tool.route_edit" => self.act_route_edit(),
            "junction.open" => self.act_junction_open(),
            "junction.member" => self.act_junction_member(p),
            "junction.style" => self.act_junction_style(p),
            "junction.wiring" => self.act_junction_wiring(p),
            "project.crossings" => self.act_crossings(p),
            // WP-S2.04 part 2. End.
            "props.show" => self.act_props_show(p),
            "props.focus" => self.act_props_focus(),
            "props.project" => self.act_props_project(),
            "select.set" => self.act_select_set(p),
            "props.row" => self.act_props_row(p),
            "props.category" => self.act_props_category(p),
            "props.edit" => self.act_props_edit(p),
            "props.input" => self.act_props_input(p),
            "props.commit" => self.act_props_commit(),
            "props.cancel" => self.act_props_cancel(),
            "props.set" => self.act_props_set(p),
            "props.choose" => self.act_props_choose(p),
            "props.row_menu" => self.act_props_row_menu(p),
            "props.action" => self.act_props_action(p),
            "props.list" => self.act_props_list(p),
            "props.mltext" => self.act_props_mltext(p),
            "preview.apply" => self.act_preview_apply(),
            "preview.cancel" => self.act_preview_cancel(),
            "toast.undo" => self.act_toast_undo(),
            "toast.dismiss" => self.act_toast_dismiss(),
            // WP-S2.05 part 2
            "find.open" => self.act_find_open(false),
            "find.open_replace" => self.act_find_open(true),
            "find.close" => self.act_find_close(),
            "find.text" => self.act_find_text(p, false),
            "find.replacement" => self.act_find_text(p, true),
            "find.option" => self.act_find_option(p),
            "find.fields" => self.act_find_fields(p),
            "find.folios" => self.act_find_folios(p),
            "find.run" => self.act_find_run(),
            "find.row" => self.act_find_row(p),
            "find.jump" => self.act_find_jump(),
            "find.replace_all" => self.act_find_replace_all(),
            "ui.describe" => Outcome::Done,
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
        self.close_project();
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
                // A recent entry goes only when its path is gone; a held lock or a bad format keeps it.
                if self.settings.recent_projects.contains(&path_text) && !dir.exists() {
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
        self.epochs += 1;
        self.project = Some(Project {
            session,
            epoch: self.epochs,
            dir: dir.to_path_buf(),
            name,
            access,
            labels,
            rows: Vec::new(),
        });
        self.install_view();
        self.settings.push_recent(&path_text);
        self.save_settings();
        Outcome::Done
    }

    fn open_for_editing(&self, dir: &Path) -> Result<(Session, OpenReport), OpenError> {
        self.open_for_editing_with(dir, Recovery::Keep)
    }

    /// The store options of this shell: the application version and the journal directory.
    fn store_options(&self) -> StoreOptions {
        let mut store = StoreOptions::new(&self.config.app_version);
        if let Some(state) = &self.config.state_dir {
            store.state_dir = Some(state.clone());
        }
        store
    }

    /// The session options of the shell's own sessions: the origin is the GUI.
    fn session_options() -> SessionOptions {
        SessionOptions {
            origin: Origin::Gui,
            ..SessionOptions::default()
        }
    }

    fn open_for_editing_with(
        &self,
        dir: &Path,
        recovery: Recovery,
    ) -> Result<(Session, OpenReport), OpenError> {
        Session::open(dir, self.store_options(), ShellModel::session_options(), recovery)
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

    /// The clean close of the open project: its journal goes and the lock is released. Callers reach it only for
    /// a clean project or after the user chose Discard. What a discard loses leaves the record too.
    fn close_project(&mut self) {
        if let Some(p) = self.project.take() {
            self.record.steps = self.record.saved_steps.clone();
            self.record.undone.clear();
            p.session.close();
        }
        self.area = None;
        self.focus = None;
        self.reset_tools();
        self.props = crate::properties::PropertiesState::default();
    }

    /// Ends the shell at the end of a run or of the window (also on a log-off): a clean project is closed, a dirty
    /// one keeps its journal. The journal of unsaved work is deleted only by a clean close or an explicit Discard;
    /// any other exit leaves it for the next open to offer (crash-recovery semantics, doc 05 section 8).
    pub fn shutdown(&mut self) {
        self.flush_layout();
        match self.project.take() {
            Some(p) if p.session.is_dirty() => drop(p),
            Some(p) => p.session.close(),
            None => {}
        }
        self.area = None;
        self.focus = None;
    }

    fn save_settings(&mut self) {
        if let Some(dir) = &self.config.config_dir
            && self.settings.save(dir).is_err()
        {
            self.banner = Some(strings::text("banner-settings-write").to_owned());
        }
    }

    fn refresh(&mut self) {
        self.refresh_rows();
    }

    fn act_save(&mut self) -> Outcome {
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: "reason-no-project",
            };
        };
        match p.session.save() {
            Ok(_) => {
                self.record.saved_steps = self.record.steps.clone();
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
                self.record.saved_steps = self.record.steps.clone();
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
        self.close_project();
        Outcome::Done
    }

    fn act_quit(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(outcome) = self.guard_unsaved("app.quit", p) {
            return outcome;
        }
        self.close_project();
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
                }
                self.refresh();
                // The tool's marks belong to the revision undone or redone (WP-S2.04 part 2, window check): the overlay
                // and the preview go; the next pointer or cursor move draws them again.
                self.tools.overlay = crate::tools::Overlay::default();
                self.tools.preview = None;
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
                // The tool's marks belong to the revision undone or redone (WP-S2.04 part 2, window check): the overlay
                // and the preview go; the next pointer or cursor move draws them again.
                self.tools.overlay = crate::tools::Overlay::default();
                self.tools.preview = None;
                Outcome::Done
            }
            Err(e) => failed("error-command", &[&e.to_string()]),
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
        self.sync_canvas_ui_scale();
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

    /// Escape (NFR-UX-07, WP-S2.03 §7.2): closes the topmost popup or dialog (the palette, then a dialog, then the
    /// link preview); with none open it sends `Escape` to the active tool, and when the tool was in its first state
    /// a tool other than Select is replaced by Select and Select's selection is cleared. A banner has a button of its
    /// own, [`ShellModel::dismiss_banner`].
    fn act_dismiss(&mut self) -> Outcome {
        if self.palette.take().is_some()
            || self.dialog.take().is_some()
            || self.tools.picker.take().is_some()
            || self.tools.junction_editor.take().is_some()
            || self.tools.link_preview.take().is_some()
        {
            return Outcome::Done;
        }
        if let Some(outcome) = self.dismiss_properties() {
            return outcome;
        }
        // WP-S2.05 part 2
        if let Some(outcome) = self.dismiss_find() {
            return outcome;
        }
        self.dismiss_tool()
    }

    /// The banner's own button.
    pub fn dismiss_banner(&mut self) {
        self.banner = None;
    }

    /// Answers the unsaved-changes dialog with Save (save, then the pending action), Discard (the pending action with
    /// `discard`) or Cancel (nothing).
    pub fn answer_confirm(&mut self, answer: ConfirmAnswer) -> Outcome {
        if matches!(self.dialog, Some(Dialog::ConfirmJournalDiscard)) {
            // The question about the earlier session's journal: Discard deletes it, anything else keeps it.
            self.dialog = None;
            return match answer {
                ConfirmAnswer::Discard => self.dispatch("file.discard_journal", &json!({ "confirm": true })),
                ConfirmAnswer::Save | ConfirmAnswer::Cancel => Outcome::Done,
            };
        }
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
        self.run_logged(cmd, params)
    }

    /// One command-log line through the session's own replay: reference resolution, preview and commit for bulk
    /// commands, one undo step, origin CmdLog (WP-S2.01 spec section 6.3); a committed step joins the record. The
    /// path of `command.run` and of the tool host's headless commits (WP-S2.03 §7.3).
    fn run_logged(&mut self, cmd: String, params: Value) -> Outcome {
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
        self.count(|c| c.run_script += 1);
        let outcome = match result {
            Ok(receipts) => {
                if let Some(primary) = receipts.last().and_then(|r| r.outcome.primary) {
                    self.tools.last_primary = serde_json::to_value(primary.1)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned));
                }
                Outcome::Done
            }
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
