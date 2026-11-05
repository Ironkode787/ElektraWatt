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
    Access, CmdError, CmdLog, CmdLogError, IdSource, OpenError, OpenReport, Origin, ReadOnlyReason, Recovery,
    SaveError, Session, SessionOptions, StoreOptions,
};
use serde_json::{Map, Value, json};

use crate::actions;
use crate::area::CanvasArea;
use crate::canvas::{CanvasBackend, HEADLESS_PANE};
use crate::first_run::FirstRun;
use crate::key;
use crate::layout::Layout;
use crate::navigator::PageNavigator;
use crate::settings::{self, Settings, UI_SCALES, scale_index};
use crate::status;
use crate::strings;
use crate::strings::Key;
use crate::wizard::Wizard;

mod canvas_actions;
// FT-408 and FT-409
mod creation;
pub mod symbol_editor;
// WP-S2.05 part 3
mod clipboard;
// WP-S2.04 part 4b: the text and graphic tools' host duties.
mod drawing;
// WP-S1.17 part 7: external change, cloud folders, .ewz, the upgrade, the merge driver
mod external;
pub(crate) use external::collision_count;
// WP-S2.04 part 4b: FT-411, Edit ▸ Bridge terminals.
mod bridge;
// G-PP triage row 7: Draw ▸ Bond.
mod bond;
// FT-1371: Draw ▸ Connect ▸ New cable… and Connect cores….
mod cables;
// WP-S2.09 part 3: the Preview tab, the renumber dialogs and the purge.
mod bulk;
mod editing;
// WP-S2.05 part 2
mod find;
mod interruption;
// WP-S2.18 part 1: joining an existing device from the window.
mod join;
pub(crate) use interruption::natural_cmp;
// WP-S2.09 part 1: the export job, the notifications, the recovery dialog.
mod jobs;
pub(crate) use jobs::file_url_of;
// WP-S4.02 part 3: placing on a panel page.
mod layout;
// WP-S2.09 part 2: the message panel.
mod messages;
mod navigation;
// WP-S2.06 part 1
mod navigators;
// WP-S2.06 part 3
mod onboarding;
// WP-S2.07 part 2: swap cores, and Delete and F2 in navigator trees (§7, FT-490).
mod row_ops;
// WP-S2.07 part 1: the strip editor's row gestures (§6.4, §6.5).
mod strip_editor;
mod table;
// WP-S3.08 part 3: a navigator table's export and import.
mod table_file;
// FT-410
mod palette;
mod parts;
mod projects;
mod properties;
pub(crate) mod tools;
// WP-S2.09 part 3: the history panel and context restore.
mod undo_panel;
// WP-S2.04 part 2: the wiring tools' host duties.
mod wiring;
mod workspaces;
// WP-S1.19 part 6: the Macros and Libraries navigators, Save as macro… and Make variable.
mod macros;
// WP-S2.07 part 4a
pub mod part_dialog;
// WP-S3.07 part 3: the print pre-dialog.
pub mod print;
// WP-S2.08 part 2: the user's keymap and the keymap editor.
mod keymaps;
// WP-S1.18 part 5
mod revision;
// WP-S2.08 part 6: the one-time tips (D17), the glossary pane (D16) and the wire-colour view (D18).
mod glossary;
mod tips;
mod wire_colours;
// WP-S6.05: the start-up offer after a crash.
pub mod startup;
pub use glossary::GlossaryRow;
pub use macros::MacroVariable;
pub use tips::Tip;

pub use creation::{FormDialog, FormField, FormRow, FormTable, PAGE_TYPES, STRUCTURE_KINDS};
pub use drawing::TextPopup;
pub use navigation::{Focus, RECENT_PAGES, SessionCalls};
// WP-S2.06 part 1
pub use navigators::{Chip, DragSource, FindingLook, MAX_CHIPS, ShownRow};
pub use row_ops::NavRename;
// FT-649
pub use parts::{MapColumn, PartsMap};
// WP-S2.09 part 3
pub use bond::BOND_KINDS;
pub use bulk::PURGE_LISTED;
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
    /// The bundled `PRIVACY.md` (WP-S6.05 D16), which the privacy buttons open; without it they open the repository's
    /// copy ([`crate::about::PRIVACY_URL`]).
    pub privacy_file: Option<PathBuf>,
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
    /// The library tiers the shell's sessions open (WP-S1.19 §11): the Macros and Libraries navigators, Save as
    /// macro… and the parts picker read them. `None`: the compiled-in libraries only, with no file read.
    pub libraries: Option<ew_api::library::LibraryTiers>,
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
            libraries: None,
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
                libraries: None,
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
            libraries: None,
        }
    }

    /// The same configuration with these library tiers (WP-S1.19 §11).
    #[must_use]
    pub fn with_libraries(mut self, libraries: Option<ew_api::library::LibraryTiers>) -> ShellConfig {
        self.libraries = libraries;
        self
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
        reason_key: Key<'static>,
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
        reason_key: Key<'static>,
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
    /// Page ▸ New page, Insert ▸ Structure node, Insert ▸ External reference (FT-408, FT-409): the fields of the
    /// action, which Enter dispatches.
    Form(FormDialog),
    /// Recover unsaved changes (WP-S2.09 part 1, D7, §6.6): opened with the read-only project when an editing open
    /// found the journal of a session that ended with unsaved changes. Recover (`journal.recover`), Open read-only
    /// (`journal.keep`, Escape) or Discard… (`file.discard_journal`); after a failed recovery only the last two.
    Recovery {
        /// The project's directory name.
        name: String,
        /// The unsaved transactions of the journal.
        transactions: usize,
        /// Whether the project on disk is still the journal's base.
        base_matches: bool,
        /// The time of the last change, epoch milliseconds (the window shows it; never in the transcript).
        last_change_ms: u64,
        /// Why the last Recover failed.
        failed: Option<String>,
    },
    /// Tools, Purge unused images (WP-S2.09 part 3, §8.6): the image files that go, which undo does not bring back;
    /// Purge (`project.assets.purge` with `confirm`) or Cancel (Escape).
    ConfirmPurge {
        /// The unused image files that are deleted, in their order.
        files: Vec<String>,
        /// The assets they hold (the parameter of the purge).
        assets: Vec<String>,
    },
    // WP-S2.07 part 4a
    /// Choose part… (D19): the part selection dialog, a 640-px form with the search, the filters, the result list
    /// and Choose.
    PartChoose(Box<part_dialog::PartChooser>),
    /// Edit part… (D20, D21): the part editor, a 640-px form with the editable members, the read-only ones and Save.
    PartEdit(Box<part_dialog::PartEditor>),
    // WP-S3.07 part 3
    /// Output ▸ Print… (D10, ruling 4 of ID-1180): the print pre-dialog, its options beside the preview of the print
    /// set, which builds as a job; Print… and Open in PDF viewer hand the built set to the window's platform.
    Print(Box<print::PrintDialog>),
    // WP-S1.17 part 7 (D24)
    /// The project changed on disk and the change collides with unsaved edits: each colliding field with its record
    /// in words, its base, mine and theirs, and the side chosen; Apply (the choices), All mine, All theirs, and Decide
    /// later (Escape: saves stay held, the banner says why).
    ExternalChange {
        /// The colliding fields.
        rows: Vec<external::CollisionRow>,
    },
    /// The first save of a project of an older format converts it (D11): Upgrade and save writes the backup first, or
    /// Cancel.
    ConfirmUpgrade {
        /// The older format.
        from: String,
    },
    /// Project ▸ Import ▸ QElectroTech project (WP-S3.09 D8): the import card (its summary and pages, Apply or Cancel),
    /// then the checkpoint confirmation (`confirm`: Create project or Cancel). Cancel writes nothing.
    ImportQet {
        /// The source's file name.
        file: String,
        /// The chosen folder's name.
        folder: String,
        /// The summary line (T5).
        summary: String,
        /// The folios of the imported pages, in page order.
        pages: Vec<String>,
        /// The imported devices.
        devices: usize,
        /// Whether the confirmation is shown.
        confirm: bool,
    },
    /// Project ▸ Import ▸ Re-import QElectroTech project (WP-S3.09 D10): a file matched to an earlier import by its
    /// items (T6): Re-import or Cancel.
    ReimportMatch {
        /// The file (never printed).
        path: String,
        /// Its file name.
        file: String,
        /// The source name of the earlier import.
        source: String,
        /// The share of the file's items the earlier import holds, in per cent.
        percent: u32,
    },
    /// The re-import card (WP-S3.09 D12): T8 and the rows of the records added, changed in the source, changed on
    /// both sides and removed in the source; Apply (one undo step) or Cancel (nothing changes).
    Reimport {
        /// The file's name.
        file: String,
        /// The summary (T8), or that there is nothing to re-import.
        summary: String,
        /// The rows, in words.
        rows: Vec<String>,
        /// Whether the re-import writes nothing (ID-1427, W52 F8): Apply is disabled.
        nothing: bool,
    },
    /// File ▸ Open .ewz…: Extract and open (into a new folder beside it), Open read-only, or Cancel.
    OpenEwz {
        /// The `.ewz` (never printed).
        path: String,
        /// Its file name.
        name: String,
    },
    /// Set up merge driver: the repository's git configuration and the project's managed blocks, after the consent
    /// (D28); Set up or Cancel.
    ConfirmGitSetup {
        /// The project folder's name, which the question names.
        folder: String,
    },
}

/// A marker line of the history (WP-S2.09 D13, view state, never a `History` entry): an export, a recovery.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HistoryMarker {
    /// The revision it belongs after.
    pub rev: u64,
    /// The text (`Exported PDF to golden.pdf`).
    pub text: String,
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
    /// An existing parts table, `parts.csv` (Import, Parts table).
    OpenPartsTable,
    /// An image file the asset store accepts: SVG, PNG or JPEG (Insert, Image).
    OpenImage,
    /// An existing keymap file (the keymap editor's Import…, WP-S2.08 D8).
    OpenKeymap,
    /// A keymap file name (the keymap editor's Export…).
    SaveKeymap,
    /// A table file name, XLSX or CSV (a navigator table's Export table…, WP-S3.08 part 3).
    SaveTable,
    /// An existing table file, XLSX or CSV (Import table…).
    OpenTable,
    /// An `.ewz` file name (File ▸ Export .ewz…, WP-S1.17 D24).
    SaveEwz,
    /// An existing `.ewz` (File ▸ Open .ewz…).
    OpenEwz,
}

/// A request to the window to open a page of the user guide in the system browser (F1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DocRequest {
    /// The page and heading below the book's output directory (`how-to/application-window.html#start-page`).
    pub target: String,
    /// The `file://` URL, when the built book is installed; `None` when it is not.
    pub url: Option<String>,
    /// The file `url` opens, below the book's directory (`nl/how-to/application-window.html` in a Dutch window that has
    /// the Dutch book); `None` when the book is not installed (FT-1267: the headless transcript's `doc`).
    pub page: Option<String>,
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
    /// The open parameter form (WP-S2.08 D5 (a)); the list is hidden while it is open.
    pub form: Option<crate::palette::PaletteForm>,
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
    /// The leftover journal that made it read-only: (transactions, base matches, last change in epoch ms).
    recovery: Option<(usize, bool, u64)>,
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
    symbol_editor: Option<symbol_editor::SymbolEditor>,
    mode: Mode,
    config: ShellConfig,
    settings: Settings,
    project: Option<Project>,
    banner: Option<String>,
    palette: Option<PaletteState>,
    /// The palette's recents (WP-S2.08 D3): the last 50 palette or shortcut invocations of this run.
    recents: crate::palette::Recents,
    /// The user's keymap (`keymap.toml`, WP-S2.08 D6), when there is one and it was not refused.
    user_keymap: Option<crate::keymap::Keymap>,
    /// Bumped when the user's keymap changes: the window rebuilds its bindings.
    keymap_version: u64,
    /// The keymap editor's state (Help ▸ Keyboard shortcuts, D8).
    keymap_editor: crate::keymap::EditorState,
    /// Whether `keymap.toml` is left as it is for this run: it could not be read, or a refused file could not be
    /// copied aside, at start-up (the editor's edits then apply without being written).
    keymap_keep_file: bool,
    dialog: Option<Dialog>,
    /// The imported project the import card shows (WP-S3.09 D8).
    import_pending: Option<projects::PendingImport>,
    /// The import job running for the import card (WP-S3.09 D4).
    import_job: Option<projects::ImportJob>,
    /// The previewed re-import the re-import card shows (WP-S3.09 D12).
    reimport_pending: Option<ew_api::import::Reimport>,
    /// The re-import job running for the re-import card (C51-p2#10).
    reimport_job: Option<projects::ReimportJob>,
    /// The project (its epoch), file and lineage of the re-import's proposal (T6) shown last: its Re-import imports
    /// into that lineage, in that project only (C52-p1#4).
    reimport_proposal: Option<(u64, PathBuf, ew_api::import::LineageInfo)>,
    /// The re-import's proposal (T6) or card that waits behind a dialog open when its job ended (C52-p1#2).
    reimport_waiting: Option<Dialog>,
    /// Bumped on every opening of a dialog: the window gives a dialog opened again, even as the same kind, its first
    /// focus (C49-p10b#4).
    dialog_gen: u64,
    prompt: Option<PromptRequest>,
    record: Record,
    quit: bool,
    origin_file: String,
    first_run: Option<FirstRun>,
    // WP-S6.05 D11
    /// The start-up offer: leftover journals and the crash log of an earlier session.
    startup: Option<startup::StartupOffer>,
    /// The log folder `startup.open_log_folder` asks the window to open: its `file://` URL and the folder.
    startup_folder_request: Option<(String, PathBuf)>,
    wizard: Option<Wizard>,
    /// The dock layout of the project view (FT-309) and the workspace it belongs to.
    layout: Layout,
    /// The bottom-dock panes `view.bottom_dock` closed, reopened by the next open (doc 16a §5.7; view state only).
    bottom_closed: Vec<crate::layout::PaneKind>,
    workspace: String,
    /// Bumped when the shell replaces the layout (a workspace was loaded or reset): the frame rebuilds its tree.
    layout_version: u64,
    /// The time (egui seconds) of the last layout change that is not on disk yet.
    layout_changed_at: Option<f64>,
    /// The documentation page F1 asked for and the window has not opened yet.
    doc_request: Option<DocRequest>,
    /// The target of the last documentation request, kept for `ui.describe`.
    last_help: Option<String>,
    /// A note the status bar's state field shows instead of "Ready" while the project it was made for is open at the
    /// revision it was made at (ID-1392, FT-1405): its text, the project's epoch and revision.
    status_note: Option<(String, Option<u64>, Option<u64>)>,
    /// The open pages as tabs in one or two groups (WP-S2.03): each tab's canvas, view memory and history.
    area: Option<CanvasArea>,
    /// The page navigator (WP-S2.03 §6.1).
    navigator: PageNavigator,
    // WP-S2.06 part 1
    /// The open navigators other than Pages, by kind.
    navs: std::collections::BTreeMap<crate::navigators::NavKind, crate::navigators::Navigator>,
    /// WP-S2.06 part 3: the Pages navigator in table mode, a generic navigator over `navigator_table(Pages)` (§8.1).
    pages_table: Option<crate::navigators::Navigator>,
    /// WP-S2.07 part 2 (FT-490): the tree row whose label is being renamed.
    rename: Option<NavRename>,
    /// FT-649: the column mapping step of the parts import.
    parts_map: Option<PartsMap>,
    /// The chips of the line "Not on this page" (§6.5), and the (selection generation, page) they were made for.
    chips: Vec<navigators::Chip>,
    chips_for: Option<(u64, Option<crate::canvas::PageId>)>,
    /// The page the focused tab showed at the end of the last dispatch (a page switch moves the Pages cursor).
    last_page: Option<crate::canvas::PageId>,
    /// Counted up by every dispatch (the navigator widget's cache key).
    nav_version: u64,
    // WP-S2.06 part 2
    /// The window's potential and net highlights (§7.3, D14): view state.
    highlights: crate::highlight::Highlights,
    /// Where the keyboard focus is.
    focus: Option<Focus>,
    /// The page content of the current revision by folio (WP-S2.03 §6.2).
    cache: navigation::PageCache,
    /// The viewer's `Session` reads, counted.
    calls: Cell<SessionCalls>,
    /// The canvas pane of a new project's first group in logical points and its pixels per point.
    canvas_pane: (f64, f64),
    canvas_ppp: f64,
    /// Whether the host has sized the first group's pane: a new area's first group is then sized (FX38d).
    canvas_sized: bool,
    /// The bottom band every canvas's fit keeps clear, in logical points (the window's tool HUD; 0 headless).
    canvas_fit_inset: f64,
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
    /// The text of the last `page.describe` step, taken by the script runner for the transcript line (WP-S6.13).
    page_text: Option<String>,
    /// The project sessions opened so far (the source of [`Project`]'s epoch).
    epochs: u64,
    /// The canvas tool host (WP-S2.03 part 2): the active tool, its overlay, the preview, the selection.
    tools: crate::tools::ToolHost,
    /// Active-tool replacement generation, independent of the replaceable host.
    tool_epoch: u64,
    /// The property grid (WP-S2.05 part 1): the sheet, the row cursor, the edit, the card and the toast.
    props: crate::properties::PropertiesState,
    /// The find pane (WP-S2.05 part 2): the query, the results, the cursor and the note.
    find: crate::find::FindState,
    // WP-S2.05 part 3
    /// The clipboard (§8.7): the headless clipboard, and what a copy hands the window.
    clip: crate::clipboard::ClipboardState,
    /// The ID aliases `@n` of this session's machine text (FT-422, ID-630); never stored.
    aliases: std::cell::RefCell<crate::alias::AliasTable>,
    /// The source of new record IDs for the sessions the shell opens or creates: `None` (the system clock and OS
    /// entropy) except in a test that replays one script under different IDs (FT-439's guard, feature `testkit`).
    id_source: Option<fn() -> Box<dyn IdSource + Send>>,
    // WP-S2.09 part 1
    /// The jobs (the PDF export) and the Jobs pane's entries.
    jobs: crate::jobs::JobsState,
    // WP-S3.07 part 3
    /// The print pre-dialog's build, its built set and the print request for the window.
    print: print::PrintState,
    /// The visible toasts and the notifications log.
    notes: crate::notifications::NotificationLog,
    /// The history's marker lines of the open project session (exports, recoveries), by revision.
    markers: Vec<HistoryMarker>,
    /// The recovery dialog that Discard… replaced with its confirmation, or the import card that the unsaved-changes
    /// question of its Apply replaced (ID-1374): Cancel returns to it.
    recovery_stash: Option<Dialog>,
    // WP-S2.09 part 2
    /// The message panel's view state: the editing page, the scheme of its table, the quick fixes' origin annotations.
    messages: crate::navigators::messages::MessagesState,
    // WP-S2.09 part 3
    /// The Preview tab's lists of the open card.
    preview: crate::preview::PreviewPane,
    /// The history panel and the contexts of the commits.
    history: crate::undo_history::HistoryPane,
    // WP-S2.17 part 2
    /// The command bar's active tab (`view.command_tab`); a session state, Draw at start.
    command_tab: crate::command_bar::Tab,
    /// Whether the command bar shows the tab row only (`view.command_bar`, Ctrl+F1).
    command_bar_collapsed: bool,
    // WP-S2.17 part 3
    /// Whether the left column is collapsed to its rail (`rail.show` on the shown entry); a session state.
    rail_collapsed: bool,
    // WP-S1.19 part 6
    /// The Macros navigator's detail and the variables of the next Save as macro….
    macros: macros::MacroState,
    // WP-S6.13 part 4
    /// Every `dispatch` since the log was last taken, in call order, while the log is on (a visual run's evidence;
    /// off by default, so the window and the headless runner keep nothing).
    dispatch_log: Option<Vec<DispatchEntry>>,
    // WP-S1.18 part 5 (cycle 41, ruling 1)
    /// Whether a person drives this shell: the window's own shell ([`ShellModel::new`] in [`Mode::Gui`]). A headless
    /// shell and a window shell a script drives ([`ShellModel::new_scripted`], the visual driver) are not, and every
    /// `HumanOnly` submission from them is refused. Set at construction only; no action or parameter changes it.
    person: bool,
    /// The revision dialogs' view state (the gate's lines of the open dialog) and the integrity items of the opened
    /// project they run the gate with.
    revision: revision::RevisionState,
    // WP-S2.08 part 6
    /// The glossary pane's search text (D16).
    glossary_filter: String,
    /// The glossary pane's row cursor (FT-1281), an index into its rows; none before the first move.
    glossary_row: Option<usize>,
    /// The point a Potentials row's Define potential… pinned for its open form (FT-1239); none otherwise.
    potential_at: Option<ew_api::output::TableEnd>,
    /// Whether the wire-colour view is on (View ▸ Wire colours, D18): view state of the session, off at start.
    wire_colours: bool,
    /// The wire-colour view's Connections table and the (session epoch, revision) it was fetched at.
    wire_colour_cache: std::cell::RefCell<Option<wire_colours::Cached>>,
    /// WP-S1.17 part 7: the cloud environment, the `ew` executable, the merge driver's status, a held change's banner.
    collab: external::CollabState,
}

/// One `dispatch` in the dispatch log (WP-S6.13 part 4 §4.3): the action as given, the outcome's word (`done`,
/// `refused`, `failed`) and a refusal's reason key. A failure's message is not kept (the log never carries a path).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DispatchEntry {
    /// The action ID as dispatched.
    pub action: String,
    /// `done`, `refused` or `failed`.
    pub outcome: &'static str,
    /// The reason key of a refusal.
    pub reason: Option<&'static str>,
}

/// A maker of [`IdSource`]s for `ShellModel::set_id_source_for_tests` (feature `testkit`).
#[cfg(feature = "testkit")]
pub type IdSourceFactory = fn() -> Box<dyn IdSource + Send>;

/// The directory's own name, never its path.
fn dir_name(dir: &Path) -> String {
    dir.file_name()
        .map_or_else(|| "project".to_owned(), |n| n.to_string_lossy().into_owned())
}

fn failed(key: Key<'_>, args: &[&str]) -> Outcome {
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
        Some(_) => Err(failed(key!("error-bad-param"), &[name])),
    }
}

/// The banner of a project open read-only because of a damaged library copy a device uses (FT-1383, ID-1418): the
/// device, the copy's part and its library when an installed library holds it; `None` for a copy no device names.
fn damaged_banner(damaged: &ew_api::DamagedCopy) -> Option<String> {
    match damaged {
        ew_api::DamagedCopy {
            device: Some(device),
            part: Some((part, library)),
            ..
        } => Some(strings::format(
            key!("banner-damaged-copy"),
            &[device, part, library],
        )),
        ew_api::DamagedCopy {
            device: Some(device),
            part: None,
            ..
        } => Some(strings::format(key!("banner-damaged-copy-unnamed"), &[device])),
        _ => None,
    }
}

fn read_only_key(reason: &ReadOnlyReason) -> Key<'static> {
    match reason {
        ReadOnlyReason::Requested => key!("readonly-requested"),
        ReadOnlyReason::LockedBy(_) => key!("readonly-locked"),
        ReadOnlyReason::NotWritable(_) => key!("readonly-not-writable"),
        ReadOnlyReason::Integrity => key!("readonly-integrity"),
        ReadOnlyReason::Format(_) => key!("readonly-format"),
        ReadOnlyReason::LockLost(_) => key!("readonly-lock-lost"),
    }
}

fn open_error_key(e: &OpenError) -> Key<'static> {
    match e {
        OpenError::Io { .. } => key!("openerr-io"),
        OpenError::NotADirectory(_) => key!("openerr-not-dir"),
        OpenError::Format(_) => key!("openerr-format"),
        OpenError::Migration(_) => key!("openerr-migration"),
        OpenError::SaveInProgress => key!("openerr-busy"),
        OpenError::NoProject(_) => key!("openerr-no-project"),
        OpenError::Model { .. } => key!("openerr-model"),
        OpenError::Takeover(_) => key!("openerr-takeover"),
        OpenError::Exists(_) => key!("openerr-exists"),
        #[allow(unreachable_patterns)]
        _ => key!("openerr-other"),
    }
}

/// The reason key of a save error (its `Display` carries file system paths that a transcript must not).
fn save_error_key(e: &SaveError) -> Key<'static> {
    match e {
        SaveError::ReadOnly(_) => key!("saveerr-read-only"),
        SaveError::LockLost(_) => key!("saveerr-lock-lost"),
        SaveError::ExternalChange { .. } => key!("saveerr-external"),
        SaveError::Failed { .. } => key!("saveerr-failed"),
        SaveError::Incomplete { .. } => key!("saveerr-incomplete"),
        SaveError::PendingIncomplete { .. } => key!("saveerr-pending"),
        SaveError::GenerationLimit => key!("saveerr-generation-limit"),
        // WP-S1.17 D21, D22: a held external change.
        SaveError::ExternalCollisions => key!("saveerr-external-collisions"),
        SaveError::ExternalUnreadable => key!("saveerr-external-unreadable"),
        _ => key!("saveerr-other"),
    }
}

#[cfg(test)]
mod save_error_tests {
    use super::{SaveError, key, save_error_key, strings};

    /// ID-1144: a save refused at the manifest's ceiling says so in words and ends with what the user can do.
    #[test]
    fn the_generation_limit_names_the_limit_and_save_as() {
        let message = strings::format(
            key!("error-save"),
            &[strings::text(save_error_key(&SaveError::GenerationLimit))],
        );
        assert_eq!(
            message,
            "Saving failed: this project has been saved the greatest number of times its format allows; save it to a \
             new folder (Save as\u{2026}), which starts the count again."
        );
    }
}

// map: closed-sets: a new closed set of choices a form dialog shows through choice_text goes in CLOSED_SETS.
/// The closed sets of choices the form dialogs show through `strings::choice_text` (ID-1278 ruling 2; FT-1262: the
/// choice-word test reads this one list): New page's types, New structure node's aspects, the yes-or-no fields,
/// Renumber devices…' scopes and schemes, Save as macro's kinds, the revision forms' roles, the part editor's kinds and a fluid line's kinds.
#[cfg(test)]
const CLOSED_SETS: &[&[&str]] = &[
    PAGE_TYPES,
    STRUCTURE_KINDS,
    bulk::BOOL_CHOICES,
    bulk::DEVICE_SCOPES,
    bulk::SCHEMES,
    &macros::MACRO_KINDS,
    revision::ROLES,
    &part_dialog::PART_KINDS,
    // WP-S1.20 T25: a fluid line's line kinds.
    crate::strings::LINE_KINDS,
];

#[cfg(test)]
mod choice_word_tests {
    use crate::strings::{self, choice_text, choice_words};

    /// ID-1278 ruling 2 (W49.1 D; C49-p6#1): every value of the closed sets the window shows through `choice_text` has
    /// its `choice-<value>` in en-GB and nl-NL: every `enum(…)` and `bool` of an `ACTIONS` signature, whatever its
    /// form plan, the form dialogs' choices (New page's types, New structure node's aspects, the yes-or-no fields,
    /// Renumber devices…' scopes and schemes, Save as macro's kinds, the revision forms' roles), the part editor's
    /// kinds and every page type of the format (the Properties pane's Kind). The en-GB words are the ones derived
    /// before; the Dutch ones their own.
    #[test]
    fn every_closed_choice_has_its_words_in_both_locales() {
        let mut values: Vec<String> = Vec::new();
        for meta in crate::actions::ACTIONS {
            for p in crate::actions::parse_signature(meta.params).unwrap_or_default() {
                if p.ty == "bool" {
                    values.extend(["false".to_owned(), "true".to_owned()]);
                } else if let Some(list) = p.ty.strip_prefix("enum(").and_then(|t| t.strip_suffix(')')) {
                    values.extend(list.split('|').map(str::to_owned));
                }
            }
        }
        // The page types of the format, from its committed schema (`report` included: generated pages show it).
        let page: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/0.1/page.schema.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let types: Vec<String> = page["$defs"]["PageType"]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["const"].as_str().map(str::to_owned))
            .collect();
        assert!(types.iter().any(|t| t == "report"), "{types:?}");
        values.extend(types);
        for set in super::CLOSED_SETS {
            values.extend(set.iter().map(|v| (*v).to_owned()));
        }
        let nl: std::collections::BTreeSet<&str> = include_str!("../locales/nl-NL/shell.ftl")
            .lines()
            .filter_map(|l| l.split_once(" = ").map(|(k, _)| k))
            .collect();
        let mut missing = std::collections::BTreeSet::new();
        for v in &values {
            let key = format!("choice-{}", v.replace('_', "-"));
            if !strings::has_key(&key) {
                missing.insert(format!("en-GB: {key}"));
            }
            if !nl.contains(key.as_str()) {
                missing.insert(format!("nl-NL: {key}"));
            }
            // C50-p5#5: the accessory has one English word, "End bracket" (16a §8 item 4).
            if v != "end_clamp" {
                assert_eq!(choice_text(v), choice_words(v), "{v}");
            }
        }
        assert_eq!(missing, std::collections::BTreeSet::new());
        assert!(strings::set_locale("nl"));
        assert_eq!(choice_text("multi_line"), "Meerlijnschema");
        assert_eq!(choice_text("end_bracket"), "Eindsteun");
        assert_eq!(choice_text("true"), "Ja");
        // An open set's value (a symbol's name) keeps its derived words in any language.
        assert_eq!(choice_words("contact_no"), "Contact no");
        assert!(strings::set_locale("en"));
    }
}

/// A new, empty directory `<temp>/<stem>-<pid>-<n>` for the unit tests of this crate: [`ew_testdir::fresh_dir`]
/// (FT-524, FT-538, FT-586).
#[cfg(test)]
pub(crate) fn fresh_test_dir(stem: &str) -> PathBuf {
    ew_testdir::fresh_dir(stem)
}

#[cfg(test)]
mod dispatch_log_tests {
    use serde_json::json;

    use super::{DispatchEntry, ShellModel};

    fn entry(action: &str, outcome: &'static str, reason: Option<&'static str>) -> DispatchEntry {
        DispatchEntry {
            action: action.to_owned(),
            outcome,
            reason,
        }
    }

    /// WP-S6.13 part 4 §4.3: the log is off by default; on, it keeps every dispatch in the order the calls started
    /// (an action that dispatches another comes first), with the outcome's word and a refusal's reason key.
    #[test]
    fn the_dispatch_log_is_off_by_default_and_keeps_every_call_in_order() {
        let mut shell = ShellModel::for_tests();
        let _ = shell.dispatch("help.about", &json!({}));
        assert!(shell.take_dispatch_log().is_empty(), "off by default");
        shell.set_dispatch_log(true);
        let _ = shell.dispatch("help.about", &json!({}));
        let _ = shell.dispatch("no.such_action", &json!({}));
        let refused = shell.dispatch("edit.undo", &json!({}));
        let super::Outcome::Refused { reason_key } = refused else {
            panic!("edit.undo without a project: {refused:?}");
        };
        shell.start_first_run_if_needed();
        let _ = shell.dispatch("first_run.set", &json!({"field": "ui_scale", "value": 1.25}));
        assert_eq!(
            shell.take_dispatch_log(),
            [
                entry("help.about", "done", None),
                entry("no.such_action", "failed", None),
                entry("edit.undo", "refused", Some(reason_key.as_str())),
                entry("first_run.set", "done", None),
                entry("view.ui_scale", "done", None),
            ]
        );
        assert!(shell.take_dispatch_log().is_empty(), "taken");
        shell.set_dispatch_log(false);
        let _ = shell.dispatch("help.about", &json!({}));
        assert!(shell.take_dispatch_log().is_empty(), "off again");
    }
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
                    banner = Some(strings::format(key!("banner-settings-bad"), &[name]));
                }
                loaded.settings
            }
            None => Settings::default(),
        };
        // WP-S2.08 part 3 (D10): the texts in the UI language of the settings.
        strings::set_locale(&settings.language);
        // WP-S2.08 D8 (KM-2): a refused keymap file is kept aside with a banner, and the preset applies.
        let mut keymap_keep_file = false;
        let user_keymap = match &config.config_dir {
            Some(dir) => match keymaps::load_user_keymap(dir, &settings.keymap) {
                Ok(k) => k,
                Err(trouble) => {
                    banner = banner.or(Some(trouble.banner));
                    keymap_keep_file = trouble.keep_file;
                    None
                }
            },
            None => None,
        };
        let workspace = settings.workspace.clone();
        let mut model = ShellModel {
            mode,
            config,
            settings,
            project: None,
            banner,
            palette: None,
            recents: crate::palette::Recents::default(),
            user_keymap,
            keymap_version: 0,
            keymap_editor: crate::keymap::EditorState::default(),
            keymap_keep_file,
            dialog: None,
            import_pending: None,
            import_job: None,
            reimport_pending: None,
            reimport_job: None,
            reimport_proposal: None,
            reimport_waiting: None,
            dialog_gen: 0,
            prompt: None,
            record: Record::default(),
            quit: false,
            origin_file: "ui-script".to_owned(),
            first_run: None,
            startup: None,
            startup_folder_request: None,
            wizard: None,
            layout: Layout::schematic(),
            bottom_closed: Vec::new(),
            workspace,
            layout_version: 0,
            layout_changed_at: None,
            doc_request: None,
            last_help: None,
            status_note: None,
            area: None,
            navigator: PageNavigator::new(),
            navs: std::collections::BTreeMap::new(),
            pages_table: None,
            rename: None,
            parts_map: None,
            chips: Vec::new(),
            chips_for: None,
            last_page: None,
            nav_version: 0,
            highlights: crate::highlight::Highlights::default(),
            focus: None,
            cache: navigation::PageCache::default(),
            calls: Cell::new(SessionCalls::default()),
            canvas_pane: HEADLESS_PANE,
            canvas_ppp: 1.0,
            canvas_sized: false,
            canvas_fit_inset: 0.0,
            grid_on: true,
            frame_stats: false,
            canvas_backend: CanvasBackend::Cpu,
            canvas_error: None,
            page_content_calls: Cell::new(0),
            page_text: None,
            epochs: 0,
            tools: crate::tools::ToolHost::new(),
            tool_epoch: 0,
            props: crate::properties::PropertiesState::default(),
            find: crate::find::FindState::default(),
            clip: crate::clipboard::ClipboardState::default(),
            aliases: std::cell::RefCell::new(crate::alias::AliasTable::new()),
            id_source: None,
            jobs: crate::jobs::JobsState::default(),
            print: print::PrintState::default(),
            notes: crate::notifications::NotificationLog::default(),
            markers: Vec::new(),
            recovery_stash: None,
            messages: crate::navigators::messages::MessagesState::default(),
            preview: crate::preview::PreviewPane::default(),
            history: crate::undo_history::HistoryPane::default(),
            command_tab: crate::command_bar::Tab::Draw,
            command_bar_collapsed: false,
            rail_collapsed: false,
            macros: macros::MacroState::default(),
            symbol_editor: None,
            dispatch_log: None,
            person: mode == Mode::Gui,
            revision: revision::RevisionState::default(),
            glossary_filter: String::new(),
            glossary_row: None,
            potential_at: None,
            wire_colours: false,
            wire_colour_cache: std::cell::RefCell::new(None),
            collab: external::CollabState::default(),
        };
        model.load_active_workspace();
        model
    }

    /// A window shell that a script drives (the visual driver, WP-S6.13): it lays out, opens dialogs and frames them
    /// as the window does ([`Mode::Gui`]), but no person is at it, so every `HumanOnly` submission from it is refused
    /// as a headless one is (cycle 41, ruling 1: a visual script opens, fills and frames the revision dialogs, and
    /// their confirm is refused). Whoever builds the shell decides this; nothing a script sends can change it.
    pub fn new_scripted(config: ShellConfig) -> ShellModel {
        let mut model = ShellModel::new(Mode::Gui, config);
        model.person = false;
        model
    }

    /// Whether a person drives this shell (see [`ShellModel::new_scripted`]).
    pub fn is_person(&self) -> bool {
        self.person
    }

    /// The origin of the shell's model commands, undo and redo: the GUI only in the window a person drives, else the
    /// script's command log (`Origin::CmdLog`: the headless shell and a scripted window, cycle 41 ruling 1);
    /// `Session::undo` and `Session::redo` refuse a `HumanOnly` step from the latter (FT-404), and every `HumanOnly`
    /// command submitted from it gets the exposure error.
    pub(super) fn origin(&self) -> Origin {
        Self::origin_of(self.mode, self.person, &self.origin_file)
    }

    /// [`ShellModel::origin`] from the fields it reads (for a caller that holds the project mutably).
    pub(super) fn origin_of(mode: Mode, person: bool, origin_file: &str) -> Origin {
        if mode == Mode::Gui && person {
            Origin::Gui
        } else {
            Origin::CmdLog {
                file: origin_file.to_owned(),
            }
        }
    }

    /// A headless shell with settings in a fresh temporary directory (unit tests of this crate).
    #[cfg(test)]
    pub(crate) fn for_tests() -> ShellModel {
        let dir = fresh_test_dir("ew-shell-test");
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

    /// Records the main window's geometry (WP-S2.17 part 3, FT-348): kept in memory and written to `settings.toml` by
    /// [`ShellModel::shutdown`], not on every move.
    pub fn set_window(&mut self, geometry: crate::window::WindowGeometry) {
        self.settings.window = Some(geometry);
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
        if let Some(editor) = &self.symbol_editor {
            return editor.can_undo();
        }
        self.project
            .as_ref()
            .is_some_and(|p| p.session.history().cursor() > 0)
    }

    /// Whether the open project has a step to redo.
    pub fn can_redo(&self) -> bool {
        if let Some(editor) = &self.symbol_editor {
            return editor.can_redo();
        }
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
            Some(AccessMode::ReadOnly { reason_key }) if reason_key == key!("readonly-recovery")
        )
    }

    /// The status bar's note (ID-1392, FT-1405): shown once, until the project changes or another one opens.
    pub fn status_note(&self) -> Option<&str> {
        let (text, epoch, rev) = self.status_note.as_ref()?;
        (*epoch == self.project.as_ref().map(|p| p.epoch) && *rev == self.revision()).then_some(text.as_str())
    }

    /// The documentation page F1 asked for, taken once by the window.
    pub fn take_doc_request(&mut self) -> Option<DocRequest> {
        self.doc_request.take()
    }

    /// The documentation page F1 asked for while the window has not taken it (the headless mode never does).
    pub fn doc_request(&self) -> Option<&DocRequest> {
        self.doc_request.as_ref()
    }

    /// The target of the last documentation request (`how-to/application-window.html#start-page`).
    pub fn last_help(&self) -> Option<&str> {
        self.last_help.as_deref()
    }

    /// The banner, if one is showing: a message, else the banner of a state that holds (saves held by a change on disk,
    /// the cloud-synced folder; W49.3-F2).
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref().or(self.collab.state_banner.as_deref())
    }

    /// The message banner alone (an action's failure or notice), without a state banner under it.
    pub fn message_banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    /// Shows `text` as the banner (a failure of an action that raised none of its own).
    pub fn show_banner(&mut self, text: &str) {
        self.banner = Some(text.to_owned());
    }

    /// The window title: `<directory name> - ElektraWatt`, with `*` before it while the project is dirty.
    pub fn title(&self) -> String {
        match &self.project {
            None => strings::text(key!("window-title-start")).to_owned(),
            Some(p) => {
                let title = strings::format(key!("window-title"), &[&p.name]);
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

    /// The generation of the dialog's opening: it changes whenever a dialog opens or is opened again (a collision dialog
    /// rebuilt by a second change), never while one is answered.
    pub fn dialog_generation(&self) -> u64 {
        self.dialog_gen
    }

    /// Whether the application should end (after `app.quit`).
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The path prompt the window must show, taken once.
    pub fn take_prompt(&mut self) -> Option<PromptRequest> {
        self.prompt.take()
    }

    /// The status-bar fields, left to right (spec section 5.3; WP-S2.09 §6.4, §6.5): with the notifications field
    /// while the log is not empty and, last, the job field while a job runs; `hide_job` leaves the job field out (the
    /// window's 200 ms rule: a job younger than that shows none; the describe text and the transcript pass `false`).
    pub fn status_fields(&self, hide_job: bool) -> Vec<String> {
        status::fields(self, hide_job)
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

    /// `text` with every UUID printed as its alias `@n` (FT-422, ID-630): the machine text of the transcript and of
    /// `ui.describe`. A UUID first seen here gets the next number (printing is an observation, so it takes `&self`; the
    /// table is a cell like the call counters).
    pub fn alias_text(&self, text: &str) -> String {
        crate::alias::alias_text(&mut self.aliases.borrow_mut(), text)
    }

    /// The UUID text (lower case) that alias `@n` names in this run, if the table holds it.
    pub fn alias_uuid(&self, n: usize) -> Option<String> {
        self.aliases.borrow().uuid(n).map(str::to_owned)
    }

    /// The alias number of `uuid` in this run, if it has been printed.
    pub fn alias_number(&self, uuid: &str) -> Option<usize> {
        self.aliases.borrow().number_of(uuid)
    }

    /// How many aliases this run has printed.
    pub fn alias_count(&self) -> usize {
        self.aliases.borrow().len()
    }

    /// Starts a new alias table: each headless script run starts one; a project opened or created within a run keeps
    /// the run's table (ID-630 (b)).
    pub fn reset_aliases(&mut self) {
        self.aliases.get_mut().clear();
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
            return Err(strings::text(key!("reason-no-project")).to_owned());
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
        // The dispatch log keeps the calls in the order they started (an action that dispatches another comes first).
        let slot = self.dispatch_log.as_mut().map(|log| {
            log.push(DispatchEntry {
                action: action.to_owned(),
                outcome: "done",
                reason: None,
            });
            log.len() - 1
        });
        let outcome = self.dispatch_unlogged(action, params);
        // W49.3-F2: a state banner follows its state, and the UI language.
        self.refresh_state_banner();
        // WP-S2.08 part 6 (D17): tip 3 on an editing tool's, Paste's, a move's or a reorder's generated-page refusal.
        self.tip_for_outcome(action, params, &outcome);
        if let Some(entry) = slot.and_then(|i| self.dispatch_log.as_mut()?.get_mut(i)) {
            (entry.outcome, entry.reason) = match &outcome {
                Outcome::Done => ("done", None),
                Outcome::Refused { reason_key } => ("refused", Some(reason_key.as_str())),
                Outcome::Failed { .. } => ("failed", None),
            };
        }
        outcome
    }

    /// Turns the dispatch log on (an empty log) or off (WP-S6.13 part 4; the visual driver).
    pub fn set_dispatch_log(&mut self, on: bool) {
        self.dispatch_log = on.then(Vec::new);
    }

    /// The dispatches logged since the last call, oldest first; empty while the log is off.
    pub fn take_dispatch_log(&mut self) -> Vec<DispatchEntry> {
        self.dispatch_log.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// [`ShellModel::dispatch`] without the dispatch log.
    fn dispatch_unlogged(&mut self, action: &actions::ActionId, params: &Value) -> Outcome {
        let Some(meta) = actions::find(action) else {
            return failed(key!("error-unknown-action"), &[action]);
        };
        let empty = Map::new();
        let p = match params {
            Value::Object(m) => m,
            Value::Null => &empty,
            _ => return failed(key!("error-params-object"), &[]),
        };
        // WP-S2.09 §6.6: while the recovery dialog is open only its answers (and what observes or ends the run) go; the
        // others refuse (with their own reason when they are disabled anyway, as the script runner's check gives it),
        // and the recovery banner stays.
        if matches!(self.dialog, Some(Dialog::Recovery { .. })) && !meta.answers_recovery {
            return Outcome::Refused {
                reason_key: (meta.enabled)(self).err().unwrap_or(key!("reason-dialog-open")),
            };
        }
        if !meta.keeps_banner {
            self.banner = None;
        }
        if let Err(reason_key) = actions::enabled_with(meta, self, p) {
            return Outcome::Refused { reason_key };
        }
        // An alias `@n` in a parameter that takes a UUID or a reference resolves before the action reads it (FT-422).
        let resolved = match crate::alias::resolve_params(&self.aliases.borrow(), meta.id, p) {
            Ok(r) => r,
            Err(alias) => return failed(key!("error-unknown-alias"), &[&alias]),
        };
        let p = resolved.as_ref().unwrap_or(p);
        if !meta.keeps_link_preview {
            self.tools.link_preview = None;
        }
        // WP-S2.09 D6: a toast stays until it is dismissed, replaced by a toast of its key or pushed out by a fourth
        // (and, in the window, for 8 seconds); a later commit only expires its Undo (WP-S2.05 §6.6's rule that the next
        // committing action removed it gives way to the stack).
        // WP-S2.09 part 2: the project session and revision before the action, for the editing page (§7.2).
        let before = self.epoch_rev();
        // WP-S2.09 §8.5: the context before the action is kept for the history entries it commits.
        let context = self.take_context();
        let outcome = (meta.handler)(self, p);
        self.keep_contexts(context);
        // The canvases follow the pages and the revision the action left (WP-S2.02 section 6.1, WP-S2.03 §6.2), and a
        // busy tool is escaped when its canvas changed (WP-S2.03 §7.2); the property grid fetches its sheet when its
        // key changed (WP-S2.05 §9 item 4).
        self.sync_views();
        self.track_editing_page(before);
        self.sync_tool_context();
        self.after_commit();
        // WP-S2.04 part 4b: a finished text edit returns to Select, the selection kept.
        self.after_text_edit();
        // WP-S2.04 part 2: the junction editor follows the revision and the page.
        self.sync_junction_editor();
        // WP-S2.04 part 3: the grips of a selected black box.
        self.sync_box_grips();
        // WP-S2.06 part 1: the navigators follow the revision, the page and the selection (§6.5).
        self.sync_navigators();
        self.sync_properties();
        self.sync_find();
        // WP-S2.09 part 3: the Preview tab follows the card, the history panel the revision.
        self.sync_preview();
        self.sync_history();
        // FT-759: a check toast whose revision is gone is taken off the screen.
        self.sync_check_toast();
        // WP-S1.19 part 6: the Macros navigator's detail follows its cursor.
        self.sync_macros();
        self.sync_library_copy();
        // FT-1251: the live region's selection words follow the revision.
        self.sync_selection_announcement();
        outcome
    }

    /// `tool.finish`: Shift+Enter or a double click; a selected free text opens text edit instead (WP-S2.04 §7.3,
    /// §10.5).
    pub(crate) fn act_tool_finish(&mut self, _p: &Map<String, Value>) -> Outcome {
        if self.tools.tool.name() == "select"
            && self.tools.tool.state() == "idle"
            && self.text_edit_target().is_ok()
        {
            self.act_text_edit()
        } else {
            self.send_tool(&crate::tools::CanvasEvent::Key(crate::tools::ToolKey::Finish))
        }
    }

    /// `text.input {text}`: the text typed into the tool.
    pub(crate) fn act_text_input(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "text") {
            Ok(Some(t)) => self.send_tool(&crate::tools::CanvasEvent::Text(t.to_owned())),
            Ok(None) => failed(key!("error-missing-param"), &["text"]),
            Err(e) => e,
        }
    }

    /// `canvas.activate_alt`: `canvas.activate` with `alt` forced.
    pub(crate) fn act_canvas_activate_alt(&mut self, p: &Map<String, Value>) -> Outcome {
        let mut alt = p.clone();
        alt.insert("alt".into(), Value::Bool(true));
        self.act_activate_with(&alt)
    }

    /// `tool.connect_on_move`: refused in this package (WP-S2.04 §8.3); the status bar says why, the window has no other
    /// trace of a key.
    pub(crate) fn act_tool_connect_on_move(&mut self, _p: &Map<String, Value>) -> Outcome {
        self.tools.notice = Some(strings::text(key!("reason-connect-on-place")).to_owned());
        self.tools.notice_sticky = false;
        Outcome::Refused {
            reason_key: key!("reason-connect-on-place"),
        }
    }

    /// `help.shortcuts`: the keymap editor opens with an empty search and nothing recording (WP-S2.08 D8).
    pub(crate) fn act_help_shortcuts(&mut self, _p: &Map<String, Value>) -> Outcome {
        self.keymap_editor = crate::keymap::EditorState::default();
        self.open_dialog(Dialog::Shortcuts)
    }

    /// `canvas.snapshot` and `canvas.preview_snapshot`: only the window and the script runner take a snapshot.
    pub(crate) fn act_canvas_snapshot(&mut self, _p: &Map<String, Value>) -> Outcome {
        failed(key!("message-no-snapshot"), &[])
    }

    /// `view.snapshot`: only the window and the script runner take a snapshot.
    pub(crate) fn act_view_snapshot(&mut self, _p: &Map<String, Value>) -> Outcome {
        failed(key!("message-no-snapshot"), &[])
    }

    /// `ui.describe`: the description is read by the caller after the dispatch.
    pub(crate) fn act_ui_describe(&mut self, _p: &Map<String, Value>) -> Outcome {
        Outcome::Done
    }

    /// `script.expect_refused`: the script runner runs it before the dispatch; a direct dispatch does not know it.
    pub(crate) fn act_script_expect_refused(&mut self, _p: &Map<String, Value>) -> Outcome {
        failed(key!("error-unknown-action"), &["script.expect_refused"])
    }

    /// `script.expect_failed` (FT-1409): the script runner runs it before the dispatch; a direct dispatch does not know
    /// it.
    pub(crate) fn act_script_expect_failed(&mut self, _p: &Map<String, Value>) -> Outcome {
        failed(key!("error-unknown-action"), &["script.expect_failed"])
    }

    /// `page.describe {folio?}` (WP-S6.13 §5.5.3): the content of the page `folio` (default the page on show) as text, read
    /// from the page-content cache (one `page_tables` call when the (revision, folio) is not in it, none when it is);
    /// the script runner takes the text with [`ShellModel::take_page_text`]. Changes no state.
    pub(crate) fn act_page_describe(&mut self, p: &Map<String, Value>) -> Outcome {
        let folio = match string_param(p, "folio") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => match self.current_folio() {
                Some(f) => f.to_owned(),
                None => {
                    return Outcome::Refused {
                        reason_key: key!("reason-no-page"),
                    };
                }
            },
        };
        if !self.page_rows().iter().any(|r| r.folio == folio) {
            return failed(key!("error-no-folio"), &[&folio]);
        }
        match self.page_for(&folio) {
            Ok(page) => {
                let mut text = crate::describe::page_describe(&page);
                // FT-621: the bridges of the strips the page draws, asked only of a page that draws a terminal (a
                // terminal CP has a side) or a strip segment.
                let draws_strip = page.tables.cps.iter().any(|c| c.side.is_some())
                    || page.tables.reps.iter().any(|r| r.rep_type == "strip_segment");
                let bridges = match &self.project {
                    Some(p) if draws_strip => p.session.bridges(),
                    _ => Vec::new(),
                };
                for line in crate::describe::page_bridge_lines(&page, &bridges) {
                    text.push('\n');
                    text.push_str(&line);
                }
                // FT-660: a generated page (a parts list and the other lists) prints its table's rows; a drawn page
                // asks nothing more of the session.
                let generated = self.page_rows().iter().any(|r| r.folio == folio && r.generated);
                let table = match &self.project {
                    Some(p) if generated => p.session.page_list_table(&folio),
                    _ => None,
                };
                for line in table.iter().flat_map(crate::describe::page_table_lines) {
                    text.push('\n');
                    text.push_str(&line);
                }
                self.page_text = Some(text);
                Outcome::Done
            }
            Err(message) => Outcome::Failed { message },
        }
    }

    /// The text `page.describe` produced in the last step, once (WP-S6.13).
    pub fn take_page_text(&mut self) -> Option<String> {
        self.page_text.take()
    }

    pub(crate) fn open_dialog(&mut self, dialog: Dialog) -> Outcome {
        self.palette = None;
        self.dialog_gen += 1;
        self.dialog = Some(dialog);
        Outcome::Done
    }

    /// The unsaved-changes guard of Open, Close and Quit: `Some` when the action must not go on now.
    fn guard_unsaved(&mut self, action: &'static str, p: &Map<String, Value>) -> Option<Outcome> {
        if !self.is_dirty() || flag(p, "discard") {
            return None;
        }
        match self.mode {
            Mode::Headless => Some(failed(key!("error-unsaved"), &[])),
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
            Mode::Headless => failed(key!("error-missing-param"), &[param]),
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

    pub(crate) fn act_open(&mut self, p: &Map<String, Value>) -> Outcome {
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

    pub(crate) fn act_open_recent(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(index) = p.get("index").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["index"]);
        };
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|i| self.settings.recent_projects.get(i))
            .cloned()
        else {
            return failed(key!("error-bad-param"), &["index"]);
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
            recovery,
        } = match opened {
            Ok(o) => o,
            Err(e) => {
                let message =
                    strings::format(key!("error-open"), &[&name, strings::text(open_error_key(&e))]);
                // A recent entry goes only when its path is gone; a held lock or a bad format keeps it.
                if self.settings.recent_projects.contains(&path_text) && !dir.exists() {
                    self.settings.remove_recent(&path_text);
                    self.save_settings();
                    self.banner = Some(strings::format(key!("banner-recent-removed"), &[&name]));
                } else {
                    self.banner = Some(message.clone());
                }
                return Outcome::Failed { message };
            }
        };
        let integrity = ew_api::integrity_items(&report);
        let findings = integrity
            .iter()
            .filter(|i| i.severity != ew_api::checks::Severity::Info)
            .count();
        self.banner = match (banner, findings) {
            (b, 0) => b,
            (None, n) => Some(strings::format_count(crate::count_key!("banner-findings"), n)),
            (Some(b), n) => Some(format!(
                "{b} {}",
                strings::format_count(crate::count_key!("banner-findings"), n)
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
        // WP-S2.09 D7: the read-only project of a leftover journal comes with the recovery dialog.
        if let Some((transactions, base_matches, last_change_ms)) = recovery {
            self.palette = None;
            self.dialog_gen += 1;
            self.dialog = Some(Dialog::Recovery {
                name: name.clone(),
                transactions,
                base_matches,
                last_change_ms,
                failed: None,
            });
        }
        self.project = Some(Project {
            session,
            epoch: self.epochs,
            dir: dir.to_path_buf(),
            name,
            access,
            labels,
            rows: Vec::new(),
        });
        // WP-S6.05 (C50-p3#9): a project the start-up offer lists is no longer offered once it is open.
        self.opened_offered_project(dir);
        // WP-S1.17 part 7: the cloud-synced folder and the older format say so; the merge driver's status is read.
        if let Some(extra) = self.opened_extras(dir, report.migrated_from.as_deref()) {
            self.banner = Some(match self.banner.take() {
                Some(b) => format!("{b} {extra}"),
                None => extra,
            });
        }
        // FT-1378 (ID-1417): each image over the pixel limit that an earlier build took in is kept, not drawn, and named.
        for extra in self.undrawn_image_notes() {
            self.banner = Some(match self.banner.take() {
                Some(b) => format!("{b} {extra}"),
                None => extra,
            });
        }
        self.install_view();
        self.settings.push_recent(&path_text);
        self.save_settings();
        // WP-S1.18 part 5: the gate's integrity findings, the release's warnings of opening (FT-842) and the offer of
        // an incomplete release.
        self.revision_opened(integrity);
        Outcome::Done
    }

    /// The open project's images that no drawing shows, one sentence each (FT-1378, decision log ID-1417): which image
    /// (the company logo, or the folios that show it), its size in pixels, that it is kept but not drawn, and the
    /// remedy.
    fn undrawn_image_notes(&self) -> Vec<String> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut notes = Vec::new();
        for image in project.session.undrawn_images() {
            let (w, h) = (image.width.to_string(), image.height.to_string());
            if image.logo {
                notes.push(strings::format(key!("banner-logo-not-drawn"), &[&w, &h]));
            }
            let key = match image.folios.len() {
                0 => continue,
                1 => key!("banner-image-not-drawn"),
                _ => key!("banner-image-not-drawn-folios"),
            };
            notes.push(strings::format(key, &[&image.folios.join(", "), &w, &h]));
        }
        notes
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
        if let Some(chain) = self.collab.migrations {
            store.migrations = chain;
        }
        if let Some(released) = self.collab.released {
            store.released = released;
        }
        store
    }

    /// The session options of the shell's own sessions: the origin is the GUI, new IDs from the system unless a test
    /// set a source (`set_id_source_for_tests`, feature `testkit`); the configuration's library tiers with the
    /// company libraries of the settings key `libraries.company` after any the configuration names (WP-S1.19 D4,
    /// FT-754 (f)).
    fn session_options(&self) -> SessionOptions {
        let libraries = self.config.libraries.clone().map(|mut tiers| {
            for dir in self.settings.company_libraries(self.config.config_dir.as_deref()) {
                if !tiers.company.contains(&dir) {
                    tiers.company.push(dir);
                }
            }
            tiers
        });
        SessionOptions {
            origin: Origin::Gui,
            ids: self.id_source.map(|make| make()),
            libraries,
            ..SessionOptions::default()
        }
    }

    /// The source of new record IDs for every session the shell opens or creates from now on (feature `testkit`
    /// only; `None`: the system clock and OS entropy, as in the product, which never sets one). FT-439's guard runs one
    /// script under an ascending and a descending source: a shell output ordered by UUID then differs (decision log
    /// ID-640). The product's IDs stay UUIDv7 from the system (ADR-0004); this is no seeded ID source for the shell
    /// (ID-630).
    #[cfg(feature = "testkit")]
    pub fn set_id_source_for_tests(&mut self, make: Option<IdSourceFactory>) {
        self.id_source = make;
    }

    /// The open project's current snapshot (feature `testkit` only): WP-S2.11 D1's AC-28 flow compares the model's
    /// records before an edit and after its undo.
    #[cfg(feature = "testkit")]
    pub fn world_for_tests(&self) -> Option<ew_api::World> {
        self.project.as_ref().map(|p| p.session.world())
    }

    fn open_for_editing_with(
        &self,
        dir: &Path,
        recovery: Recovery,
    ) -> Result<(Session, OpenReport), OpenError> {
        let mut store = self.store_options();
        // D25: a half-synced cloud folder is read again before a missing file makes it read-only.
        store.cloud_synced = self.cloud_of(dir).is_some();
        Session::open(dir, store, self.session_options(), recovery)
    }

    /// Decides the access of a freshly opened session: a leftover journal re-opens it read-only.
    fn settle_open(
        &self,
        dir: &Path,
        session: Session,
        report: OpenReport,
        read_only: bool,
    ) -> Result<Opened, OpenError> {
        if !read_only && let Some(info) = &report.recovery {
            // The editing session is closed again (its own journal goes, the leftover one stays) and the project
            // re-opens through `open_read_only`; the recovery dialog answers the rest (WP-S2.09 D7).
            let recovery = Some((info.transactions, info.base_matches, info.last_change_ms));
            session.close();
            let (again, again_report) = Session::open_read_only(dir, &self.config.app_version)?;
            return Ok(Opened {
                session: again,
                report: again_report,
                access: AccessMode::ReadOnly {
                    reason_key: key!("readonly-recovery"),
                },
                banner: Some(strings::text(key!("banner-recovery")).to_owned()),
                recovery,
            });
        }
        Ok(match &report.access {
            Access::Edit => Opened {
                session,
                report,
                access: AccessMode::Edit,
                banner: None,
                recovery: None,
            },
            Access::ReadOnly(reason) => {
                let key = read_only_key(reason);
                // FT-1383 (ID-1418): a damaged library copy a device uses is named, with the way out.
                let damaged = matches!(reason, ReadOnlyReason::Integrity)
                    .then(|| session.damaged_copy())
                    .flatten();
                let banner = (!read_only).then(|| {
                    damaged
                        .as_ref()
                        .and_then(damaged_banner)
                        .unwrap_or_else(|| strings::format(key!("banner-read-only"), &[strings::text(key)]))
                });
                Opened {
                    session,
                    report,
                    access: AccessMode::ReadOnly { reason_key: key },
                    banner,
                    recovery: None,
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
        self.parts_map = None;
        // WP-S2.09: the toasts of the project go with it (the log keeps them; their Undo expires with the session), and
        // so do its history markers.
        self.notes.clear_visible();
        self.markers.clear();
        // WP-S2.09 part 3: the Preview tab's lists and the history panel's state are the project's.
        self.preview.clear();
        self.history.clear();
        // WP-S1.18 part 5 and FT-842: the revision dialogs' state and the release's warnings are the project's.
        self.revision = revision::RevisionState::default();
        self.messages.release_notes = None;
        // WP-S3.09 D9 d: the migration report is the choice of the project it was made for.
        self.messages.migration = false;
        // C52-p1#4, #2: the re-import's proposal and a waiting proposal or card are the project's.
        self.reimport_proposal = None;
        self.reimport_waiting = None;
    }

    /// Ends the shell at the end of a run or of the window (also on a log-off): a clean project is closed, a dirty
    /// one keeps its journal. The journal of unsaved work is deleted only by a clean close or an explicit Discard;
    /// any other exit leaves it for the next open to offer (crash-recovery semantics, doc 05 section 8).
    pub fn shutdown(&mut self) {
        self.flush_layout();
        // WP-S2.09: a running export is cancelled and waited for (it stops at its next page), so no job outlives the
        // shell and no sibling file is left by a killed thread. Its real end is recorded like any other: its entry,
        // its notification in the log, its marker (ID-719: Quit cancels running jobs). A headless run waits for its
        // jobs before it gets here (`wait_jobs`).
        for (number, end) in self.jobs.stop_all() {
            let _ = self.finish_job(number, end);
        }
        // WP-S3.07 part 3: a print set still building is cancelled and waited for; it wrote no file.
        self.stop_print();
        match self.project.take() {
            Some(p) if p.session.is_dirty() => drop(p),
            Some(p) => p.session.close(),
            None => {}
        }
        self.area = None;
        self.focus = None;
        // FT-348: the window's geometry, recorded only by a real window, is written once at its end, merged into the
        // file as it is on disk now (R1-5): another instance may have changed other settings meanwhile.
        if self.mode == Mode::Gui
            && let (Some(geometry), Some(dir)) = (self.settings.window, &self.config.config_dir)
            && Settings::save_window(dir, geometry).is_err()
        {
            self.banner = Some(strings::text(key!("banner-settings-write")).to_owned());
        }
    }

    fn save_settings(&mut self) {
        if let Some(dir) = &self.config.config_dir
            && self.settings.save(dir).is_err()
        {
            self.banner = Some(strings::text(key!("banner-settings-write")).to_owned());
        }
    }

    fn refresh(&mut self) {
        self.refresh_rows();
    }

    pub(crate) fn act_save(&mut self) -> Outcome {
        if self.symbol_editor.is_some() {
            return self.act_symbol("save", &Map::new());
        }
        self.save_project()
    }

    fn save_project(&mut self) -> Outcome {
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        match p.session.save() {
            Ok(saved) => {
                self.record.saved_steps = self.record.steps.clone();
                // WP-S2.09 §8.4: a save is a marker line of the history panel.
                let rev = p.session.world().rev.0;
                self.markers.push(HistoryMarker {
                    rev,
                    text: strings::text(key!("marker-saved")).to_owned(),
                });
                // FT-1154 (WP-S6.08 D3): every save that wrote an unreleased format says so.
                if saved.unstable {
                    self.notify_toned(
                        strings::text(key!("toast-saved-unreleased-format")).to_owned(),
                        crate::notifications::NoteTone::Warning,
                    );
                }
                Outcome::Done
            }
            // WP-S1.17 D11, D24: the first save of a project of an older format asks once.
            Err(SaveError::UpgradeConsent { from }) => self.ask_upgrade(&from),
            Err(e) => {
                let message = strings::format(key!("error-save"), &[strings::text(save_error_key(&e))]);
                self.banner = Some(message.clone());
                // After Decide later (D24), a save held by the collisions shows their dialog again.
                if matches!(e, SaveError::ExternalCollisions) && self.dialog.is_none() {
                    self.open_collisions();
                }
                Outcome::Failed { message }
            }
        }
    }

    pub(crate) fn act_save_as(&mut self, p: &Map<String, Value>) -> Outcome {
        // FT-1383 (ID-1418): a project whose damaged library copy a device uses is not copied: the copy would not open.
        if let Some(ew_api::DamagedCopy {
            device: Some(device),
            part,
            ..
        }) = self.project.as_ref().and_then(|p| p.session.damaged_copy())
        {
            let message = match part {
                Some((part, library)) => {
                    strings::format(key!("error-save-as-damaged-copy"), &[&device, &part, &library])
                }
                None => strings::format(key!("error-save-as-damaged-copy-unnamed"), &[&device]),
            };
            self.banner = Some(message.clone());
            return Outcome::Failed { message };
        }
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
                reason_key: key!("reason-no-project"),
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
                let message = strings::format(key!("error-save-as"), &[strings::text(open_error_key(&e))]);
                self.banner = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    pub(crate) fn act_close(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(outcome) = self.guard_unsaved("file.close", p) {
            return outcome;
        }
        self.close_project();
        Outcome::Done
    }

    pub(crate) fn act_quit(&mut self, p: &Map<String, Value>) -> Outcome {
        if self
            .symbol_editor
            .as_ref()
            .is_some_and(symbol_editor::SymbolEditor::dirty)
        {
            return failed(key!("symbol-unsaved"), &[]);
        }
        if let Some(outcome) = self.guard_unsaved("app.quit", p) {
            return outcome;
        }
        self.close_project();
        self.quit = true;
        Outcome::Done
    }

    pub(crate) fn act_undo(&mut self) -> Outcome {
        if self.symbol_editor.is_some() {
            return self.act_symbol("undo", &Map::new());
        }
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let origin = Self::origin_of(self.mode, self.person, &self.origin_file);
        let before = p.session.history().cursor();
        match p.session.undo(&origin) {
            Ok(_) => {
                let after = p.session.history().cursor();
                // WP-S2.09 §8.5: the first entry of the step undone.
                let crossed = (after < before)
                    .then(|| p.session.history().entries().get(after).map(|e| e.rev().0))
                    .flatten();
                if after < before
                    && let Some(step) = self.record.steps.pop()
                {
                    self.record.undone.push(step);
                }
                self.refresh();
                // The tool's marks belong to the revision undone or redone (WP-S2.04 part 2, window check): the overlay
                // and the preview go; the next pointer or cursor move draws them again.
                self.tools.overlay = crate::tools::Overlay::default();
                self.tools.preview = None;
                self.crossed_step(crossed);
                // FT-1083: a terminal placement takes the strip's next designation again.
                self.tools.reread_designation = true;
                Outcome::Done
            }
            Err(CmdError::Exposure { .. }) => Outcome::Refused {
                reason_key: key!("reason-human-only-history"),
            },
            Err(e) => tools::command_failed(&e, &Value::Null),
        }
    }

    pub(crate) fn act_redo(&mut self) -> Outcome {
        if self.symbol_editor.is_some() {
            return self.act_symbol("redo", &Map::new());
        }
        let Some(p) = &mut self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        let origin = Self::origin_of(self.mode, self.person, &self.origin_file);
        let before = p.session.history().cursor();
        match p.session.redo(&origin) {
            Ok(_) => {
                let after = p.session.history().cursor();
                // WP-S2.09 §8.5: the first entry of the step redone.
                let crossed = (after > before)
                    .then(|| p.session.history().entries().get(before).map(|e| e.rev().0))
                    .flatten();
                if after > before
                    && let Some(step) = self.record.undone.pop()
                {
                    self.record.steps.push(step);
                }
                self.refresh();
                // The tool's marks belong to the revision undone or redone (WP-S2.04 part 2, window check): the overlay
                // and the preview go; the next pointer or cursor move draws them again.
                self.tools.overlay = crate::tools::Overlay::default();
                self.tools.preview = None;
                self.crossed_step(crossed);
                // FT-1083: a terminal placement takes the strip's next designation again.
                self.tools.reread_designation = true;
                Outcome::Done
            }
            Err(CmdError::Exposure { .. }) => Outcome::Refused {
                reason_key: key!("reason-human-only-history"),
            },
            Err(e) => tools::command_failed(&e, &Value::Null),
        }
    }

    pub(crate) fn act_ui_scale(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(scale) = p.get("scale").and_then(Value::as_f64) else {
            return failed(key!("error-missing-param"), &["scale"]);
        };
        let scale = scale as f32;
        if scale_index(scale).is_none() {
            return failed(key!("error-bad-param"), &["scale"]);
        }
        self.settings.ui_scale = UI_SCALES[scale_index(scale).unwrap_or(0)];
        self.save_settings();
        self.sync_canvas_ui_scale();
        Outcome::Done
    }

    pub(crate) fn act_palette(&mut self, p: &Map<String, Value>) -> Outcome {
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
                form: None,
            });
        }
        Outcome::Done
    }

    /// Escape (NFR-UX-07, WP-S2.03 §7.2): closes the topmost popup or dialog (the palette, then a dialog, then the
    /// link preview); with none open it sends `Escape` to the active tool, and when the tool was in its first state
    /// a tool other than Select is replaced by Select and Select's selection is cleared. A banner has a button of its
    /// own, [`ShellModel::dismiss_banner`].
    pub(crate) fn act_dismiss(&mut self) -> Outcome {
        // WP-S2.09 §6.6: Escape in the confirmation that Discard… opened returns to the recovery dialog.
        // ID-1374: and Escape in the unsaved-changes question of the import card's Apply returns to the card.
        if matches!(
            self.dialog,
            Some(Dialog::ConfirmJournalDiscard | Dialog::ConfirmDiscard { .. })
        ) && let Some(stashed) = self.recovery_stash.take()
        {
            self.dialog_gen += 1;
            self.dialog = Some(stashed);
            return Outcome::Done;
        }
        // WP-S2.08 D5 (a): Escape in a parameter form returns to the palette's list.
        if self.close_palette_form() {
            return Outcome::Done;
        }
        // WP-S2.08 D8: Escape while the keymap editor records ends the recording and leaves the dialog open.
        if matches!(self.dialog, Some(Dialog::Shortcuts)) && self.keymap_editor.recording.take().is_some() {
            return Outcome::Done;
        }
        // WP-S3.07 part 3 (ruling 4 of ID-1180): Escape and Cancel in the print pre-dialog cancel its build and print
        // nothing.
        if self.palette.is_none() && matches!(self.dialog, Some(Dialog::Print(_))) {
            self.drop_print();
        }
        if self.palette.take().is_some()
            || self.dialog.take().is_some()
            // WP-S2.09 §6.5: the notifications log is a popup.
            || std::mem::take(&mut self.notes.popup)
            // WP-S2.09 §8.4: so is Show changes.
            || self.history.changes.take().is_some()
            || self.tools.picker.take().is_some()
            // WP-S2.18 D2: Escape closes the Join chooser and leaves the target as it was.
            || self.tools.chooser.take().is_some()
            // FT-649: Escape closes the parts import's column mapping step.
            || self.parts_map.take().is_some()
            || self.tools.junction_editor.take().is_some()
            || self.tools.link_preview.take().is_some()
        {
            return Outcome::Done;
        }
        if let Some(outcome) = self.dismiss_properties() {
            return outcome;
        }
        // WP-S2.06 part 3: Escape in a navigator's table ends the cell edit or the pattern dialog, then the table card.
        if let Some(Focus::Navigator(k)) = self.focus
            && let Some(outcome) = self.dismiss_table(k)
        {
            return outcome;
        }
        // C49-p8b#2: Escape in a navigator with an empty filter ends the strip editor's scope.
        if let Some(Focus::Navigator(k)) = self.focus
            && let Some(n) = self.generic_mut(k)
            && n.filter.is_empty()
            && n.scope.take().is_some()
        {
            return Outcome::Done;
        }
        // WP-S2.06 part 1: Escape in a navigator cancels the reorder card (WP-S2.09 part 2: and the quick-fix card;
        // WP-S3.08 part 3: and the table import card).
        if matches!(self.focus, Some(Focus::Navigator(_)))
            && self.props.card().is_some_and(|c| {
                matches!(
                    c.origin,
                    crate::properties::CardOrigin::Reorder { .. }
                        | crate::properties::CardOrigin::QuickFix { .. }
                        | crate::properties::CardOrigin::TableImport(_)
                )
            })
        {
            self.props.card = None;
            return Outcome::Done;
        }
        // WP-S2.05 part 2
        if let Some(outcome) = self.dismiss_find() {
            return outcome;
        }
        // FX38d (doc 16a §5.7): Escape with the focus in the Messages pane closes it (the focus returns to the canvas),
        // once a tool at work has had its Escape (the Select tool at rest).
        if self.focus == Some(Focus::Navigator(crate::navigators::NavKind::Messages))
            && self.tools.tool.name() == "select"
            && self.tools.tool.state() == "idle"
        {
            return self.dispatch("messages.show", &serde_json::json!({ "on": false }));
        }
        self.dismiss_tool()
    }

    /// The banner's own button.
    pub fn dismiss_banner(&mut self) {
        if self.banner.take().is_none() {
            self.dismiss_state_banner();
        }
    }

    /// Answers the unsaved-changes dialog with Save (save, then the pending action), Discard (the pending action with
    /// `discard`) or Cancel (nothing).
    pub fn answer_confirm(&mut self, answer: ConfirmAnswer) -> Outcome {
        if matches!(self.dialog, Some(Dialog::ConfirmJournalDiscard)) {
            // The question about the earlier session's journal: Discard deletes it, anything else keeps it (and returns
            // to the recovery dialog it came from, WP-S2.09 §6.6).
            self.dialog = None;
            let stashed = self.recovery_stash.take();
            return match answer {
                ConfirmAnswer::Discard => self.dispatch("file.discard_journal", &json!({ "confirm": true })),
                ConfirmAnswer::Save | ConfirmAnswer::Cancel => {
                    self.dialog_gen += 1;
                    self.dialog = stashed;
                    Outcome::Done
                }
            };
        }
        let Some(Dialog::ConfirmDiscard { pending }) = self.dialog.clone() else {
            return Outcome::Done;
        };
        self.dialog = None;
        // ID-1374: the import card that the question replaced comes back, for Cancel and for the Apply that follows.
        if let Some(card) = self.recovery_stash.take() {
            self.dialog_gen += 1;
            self.dialog = Some(card);
        }
        let mut params = pending.params.as_object().cloned().unwrap_or_default();
        match answer {
            ConfirmAnswer::Cancel => Outcome::Done,
            ConfirmAnswer::Discard => {
                params.insert("discard".to_owned(), Value::Bool(true));
                self.dispatch(pending.action, &Value::Object(params))
            }
            ConfirmAnswer::Save => match self.save_project() {
                Outcome::Done => self.dispatch(pending.action, &Value::Object(params)),
                other => other,
            },
        }
    }

    pub(crate) fn act_command_run(&mut self, p: &Map<String, Value>) -> Outcome {
        let cmd = match string_param(p, "cmd") {
            Err(e) => return e,
            Ok(Some(cmd)) => cmd.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["cmd"]),
        };
        let params = match p.get("params") {
            None | Some(Value::Null) => json!({}),
            Some(v @ Value::Object(_)) => v.clone(),
            Some(_) => return failed(key!("error-bad-param"), &["params"]),
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
            Err(e) => return failed(key!("error-command"), &[&e.to_string()]),
        };
        let file = self.origin_file.clone();
        let Some(project) = &mut self.project else {
            return Outcome::Refused {
                reason_key: key!("reason-no-project"),
            };
        };
        // FT-1425 (ID-1395): the `library.embed` that a `plc.set_profile` naming a library's profile runs first (WP-S7.03
        // D8), an undo step of its own, joins the record before it, so the recorded script replays without the library.
        let embed = (cmd == "plc.set_profile")
            .then(|| params.get("profile").and_then(Value::as_str))
            .flatten()
            .and_then(|p| project.session.profile_embed(p));
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
            // The one line's command error as every other shell path shows it (FT-779); a malformed line or an
            // unresolved reference as the log prints it.
            Err(CmdLogError::Failed { error, .. } | CmdLogError::Refused { error, .. }) => {
                tools::command_failed(&error, &params)
            }
            Err(e) => failed(key!("error-command"), &[&e.to_string()]),
        };
        if committed && outcome == Outcome::Done {
            self.record.undone.clear();
            if let Some(embed) = embed {
                self.record.steps.push(RecordedCommand {
                    cmd: embed.name.to_string(),
                    params: embed.params,
                });
            }
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
