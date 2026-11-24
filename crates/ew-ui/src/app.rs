// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui frame over the shell model: the command bar with the app menu,
//! the banner, the start page, the first-run screen, the wizard, or the docked project view (the
//! page list and the page canvas, with the tool HUD and the zoom control over it), the status bar, the
//! palette and the dialogs, and the routing of keyboard shortcuts.
//!
//! The frame paints from `ShellModel`'s getters and sends everything the user does to `ShellModel::dispatch`
//! (a menu item, a palette entry, a shortcut, a dialog answer, a file dialog's result). It names no `Session`.
//! `ew-ui` does not depend on eframe: the application binary calls [`App::ui`] from its `eframe::App`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{CentralPanel, Key, Modifiers, Panel, Ui, ViewportCommand};
use ew_viewmodel::actions::Context;
use ew_viewmodel::key;
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::shell::Screen;
use ew_viewmodel::shell::print::{PrintEnd, PrintRequest};
use ew_viewmodel::shell::{ConfirmAnswer, Focus, Outcome, Prompt, PromptRequest, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::canvas::{CanvasCounters, CanvasHost};
use crate::canvas_area::CanvasAreaHost;
use crate::command_bar::CommandBarHost;
use crate::dock::Dock;
use crate::find::FindPane;
use crate::gpu::GpuHost;
use crate::keys::{self, Binding};
use crate::navigator::NavigatorPane;
use crate::page_view::PageView;
use crate::properties::PropertiesPane;
use crate::{about, first_run, palette, start_page, status_bar, wizard};

mod frame;
mod key_routing;
mod requests;
mod tool_options;

/// What the user did in one frame, applied after the frame is painted.
#[derive(Clone, Debug)]
pub(crate) enum Event {
    /// An action of the registry, with its parameters.
    Action(&'static str, Value),
    /// Enter in the palette.
    PaletteRun,
    /// A click on a palette entry.
    PaletteClick(usize),
    /// The palette's text changed.
    PaletteQuery(String),
    /// An arrow key in the palette.
    PaletteMove(isize),
    /// A row of the palette's parameter form changed: its index and its text.
    PaletteField(usize, String),
    /// An answer to the unsaved-changes dialog.
    Confirm(ConfirmAnswer),
    /// A symbol editor action that drops its draft, asked first when the draft has unsaved changes.
    SymbolAsk(&'static str, Value),
    /// The go-to field changed.
    GotoText(String),
    /// Go in the go-to dialog: a folio or a cell.
    GotoGo(String),
    /// The save-layout field changed.
    WorkspaceText(String),
    /// A field of a creation dialog changed: the parameter and its text.
    FormField(&'static str, String),
    /// Enter in a creation dialog, or its Create button.
    FormSubmit,
    /// A cell of the form's table changed: the row, the column (`None`: the first, a rename) and its text.
    FormTableCell(usize, Option<usize>, String),
    /// The form table's Add.
    FormTableAdd,
    /// The form table's remove button of a row.
    FormTableRemove(usize),
    /// The banner's own button.
    DismissBanner,
    /// The privacy button of the About dialog and of the first-run privacy step: `PRIVACY.md` in the system's viewer
    /// (the bundled copy when there is one).
    OpenPrivacy,
    /// The junction sequence editor's focused row (a view change of the popup).
    JunctionRow(usize),
    /// A toast's Close: the toast leaves the screen, the log keeps it.
    DismissToast(u64),
    /// A click on a disabled command: its reason in words, as a toast.
    DisabledClick(String),
}

/// What a file dialog came back with.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pick {
    /// The path the user chose.
    Path(PathBuf),
    /// The user cancelled.
    Cancelled,
    /// There is no file dialog on this system (Linux without an xdg desktop portal): the frame tells the user.
    Unavailable,
}

/// How a file dialog's answer comes to the frame.
#[derive(Debug)]
pub enum PickReply {
    /// At once: a test's platform, or a system without a file dialog.
    Now(Pick),
    /// From the dialog's own thread: the frame reads the receiver at the start of every frame, and the thread
    /// requests a repaint once the answer is in it ([`spawn_pick`]).
    Later(Receiver<Pick>),
}

/// Runs `ask` (a native file dialog) on a thread of its own and returns the receiver of its answer; the thread sends
/// the answer and then requests a repaint of `ctx`, so the frame that the request starts finds it.
///
/// A modal dialog run inside the frame spins its own message loop on the window's thread. On Windows that loop
/// dispatches the repaint which egui requested at the start of the frame to winit, which can only buffer it (its
/// handler is busy with the frame) and delivers its buffer only when the event loop wakes again; egui sends no second
/// request while that one is outstanding, so the answer stayed unpainted until the next pointer input. A dialog off
/// the window's thread leaves the event loop running.
///
/// # Errors
///
/// The thread could not be started.
pub fn spawn_pick(
    ctx: &egui::Context,
    ask: impl FnOnce() -> Pick + Send + 'static,
) -> std::io::Result<Receiver<Pick>> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("ew-file-dialog".to_owned())
        .spawn(move || {
            let pick = ask();
            // The answer first, then the wake-up: the frame the request starts must find it. A frame that has gone
            // (the window closed) leaves nobody to receive it.
            let _ = tx.send(pick);
            ctx.request_repaint();
        })?;
    Ok(rx)
}

/// The operating system's file dialogs and browser, which the application binary provides (`rfd` and the system's
/// opener) and tests replace.
pub trait Platform {
    /// Asks the user for a path: a project directory, a new directory, a PDF file or a QElectroTech file.
    fn pick(&mut self, prompt: Prompt) -> Pick;

    /// Starts the file dialog of `prompt` and says how its answer comes. The default answers at once with
    /// [`Platform::pick`]; the application binary runs its native dialogs through [`spawn_pick`], so the window keeps
    /// its event loop while a dialog is open. `ctx` is the context to repaint when the answer is there.
    fn start_pick(&mut self, prompt: Prompt, ctx: &egui::Context) -> PickReply {
        let _ = ctx;
        PickReply::Now(self.pick(prompt))
    }

    /// Opens a URL in the system browser: a `file://` URL of the installed user guide, or the repository's
    /// `PRIVACY.md` (a user action; the application itself sends nothing); `false` when it could not be opened.
    fn open_url(&mut self, _url: &str) -> bool {
        false
    }

    /// The file of the guide's page `target` (`how-to/plc-workspace.html#…`) that a run without a browser names instead
    /// of opening it (the offscreen visual driver: under its `book/` folder); `None` where a guide page is opened.
    fn guide_file(&self, _target: &str) -> Option<String> {
        None
    }

    /// Opens a written file (an exported PDF or table) in the system's default application for its type, by its path;
    /// `false` when it could not be opened.
    fn open_file(&mut self, _file: &std::path::Path) -> bool {
        false
    }

    /// Opens the project directory `project` in a second window of the program (a tutorial's practice project, while
    /// this window keeps its project); `false` when it could not be started.
    fn open_window(&mut self, _project: &std::path::Path) -> bool {
        false
    }

    /// Prints a built print set: `request` holds the PDF, the title and
    /// whether Print… or Open in PDF viewer asked. The application binary prints with `ew-print` on a worker thread
    /// and answers later; `ctx` is the context to repaint when the end is there. The default refuses: printing is not
    /// available in this window.
    fn print(&mut self, request: PrintRequest, ctx: &egui::Context) -> PrintReply {
        let _ = (request, ctx);
        PrintReply::Now(PrintEnd::Unavailable)
    }
}

/// Whether `ui.dismiss` goes to an open menu (item 5): while one is open the menu (with its
/// submenus) closes and nothing else runs, as Escape does. The one place the window's event loop and a visual script's
/// action line both reach.
pub(crate) fn menu_takes_dismiss(ctx: &egui::Context) -> bool {
    // Any open menu, a context menu among them, takes it, not the app menu alone.
    if egui::Popup::is_any_open(ctx) {
        egui::Popup::close_all(ctx);
        return true;
    }
    false
}

/// How the end of a print comes to the frame.
#[derive(Debug)]
pub enum PrintReply {
    /// At once: the platform's default, or a print that could not start.
    Now(PrintEnd),
    /// From the print's own thread: the frame reads the receiver at the start of every frame.
    Later(Receiver<PrintEnd>),
}

/// A platform whose dialogs are always cancelled.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPlatform;

impl Platform for NoPlatform {
    fn pick(&mut self, _: Prompt) -> Pick {
        Pick::Cancelled
    }
}

/// The application frame: the shell model and what the window needs besides it.
pub struct App {
    shell: ShellModel,
    platform: Box<dyn Platform>,
    /// The clock the window runs on: the wall clock, or the frame clock of a visual run.
    clock: crate::clock::Clock,
    /// The page canvases of the project view.
    canvas: CanvasHost,
    /// The canvas area's nested tab tree.
    canvas_area: CanvasAreaHost,
    /// The page navigator pane.
    navigator: NavigatorPane,
    /// The panes of the other open navigators, by kind.
    navigators: std::collections::BTreeMap<ew_viewmodel::navigators::NavKind, NavigatorPane>,
    /// The property grid pane.
    properties: PropertiesPane,
    /// The find pane.
    find: FindPane,
    /// The toasts over the frames: their times, fades and place.
    toasts: crate::notifications::ToastHost,
    /// The running jobs by number and the frame time (egui's `input.time`, seconds) the window first saw each (the
    /// elapsed time and the 200 ms rule; the frame clock, like the toasts').
    job_started: std::collections::BTreeMap<u64, f64>,
    /// Whether the shell has the window's repaint request as its jobs' waker.
    waker_set: bool,
    /// The canvas area's splitter handles on screen in the last frame.
    area_splitters: usize,
    /// The page view of the wizard's preview.
    preview_view: PageView,
    symbol_editor_view: crate::symbol_editor::EditorView,
    dock: Dock,
    initial: Option<PathBuf>,
    bindings: Vec<Binding>,
    /// The keymap the bindings were built for: the preset and the user keymap's version.
    bindings_for: (ew_viewmodel::keymap::Preset, u64),
    title: String,
    /// The Join chooser's type-ahead text.
    chooser_typing: crate::chooser::TypeAhead,
    /// The file dialog that is open off the window's thread, with the request it answers.
    pending_pick: Option<(PromptRequest, Receiver<Pick>)>,
    /// The prints running off the window's thread, each with the receiver of its end.
    pending_prints: Vec<Receiver<PrintEnd>>,
    /// The `ui_scale` last given to egui: the zoom is set when the setting changes, not every frame, so that
    /// egui's own zoom keys keep working in between.
    applied_scale: Option<f32>,
    /// The zoom factor of the previous frame, for the window memory (its report of a frame uses that zoom).
    window_zoom: Option<f32>,
    /// The theme, density, minimum text size and reduce-motion settings last given to egui.
    applied_theme: Option<(String, String, u8, bool)>,
    /// Edit ▸ Paste (or the palette) asked for the system clipboard: the frame sends `ViewportCommand::RequestPaste`,
    /// and the platform answers with a paste event, which runs `edit.paste_clip` with its text like Ctrl+V.
    request_paste: bool,
    /// The system clipboard was asked for by Edit ▸ Paste in place: the platform's paste event in the next frame runs
    /// `edit.paste_in_place` with its text instead of `edit.paste_clip`.
    paste_in_place: bool,
    /// The IP tool's name field in the tool strip: `ip.name` on Enter.
    ip_name: String,
    /// The Wire marker tool's cross-section and colours fields: `marker.fields` on Enter.
    marker_csa: String,
    marker_colours: String,
    /// The Structure box tool's caption and location fields: `structure_box.fields` on Enter.
    structure_caption: String,
    structure_location: String,
    /// The tool generation whose drawing-field drafts are displayed.
    drawing_fields_epoch: Option<u64>,
    /// The text popup's field: `None` while the popup is closed.
    text_buffer: Option<String>,
    /// The command bar's window state (the collapsed bar's popover, the search field's place).
    command_bar: CommandBarHost,
    /// The page view pane's rectangle in the last frame: the HUD, the zoom control and the tool options stand on it.
    page_pane: Option<egui::Rect>,
    /// The pane of the station's view on show in the last frame; `None` while a page is on show.
    station_pane: Option<egui::Rect>,
    /// The HUD's rectangle in the last frame.
    hud_rect: Option<egui::Rect>,
    /// The tool options card's rectangle in the last frame (where the ghost is under it).
    options_rect: Option<egui::Rect>,
    /// The band of the tool options card's controls in the last frame: the card does not fade while the pointer is
    /// on it.
    options_band: Option<egui::Rect>,
    /// Whether the window had the operating system's focus in the last frame.
    window_focused: Option<bool>,
    /// How many times the window regained the focus and asked for the frames that repaint it (tests).
    focus_repaints: u64,
    /// When the New project dialog's folders were last read again, on the window's clock.
    new_project_checked: f64,
    /// When the project on disk was last polled for an external change, on the window's clock, and the
    /// time to the next poll in seconds.
    disk_polled: (f64, f64),
    /// Alt is down and nothing else was pressed since: its release opens the app menu (doc 16a §5.2).
    alt_alone: bool,
    /// Whether Alt was down in the last frame.
    alt_down: bool,
    /// The actions the frame dispatched, oldest first, at most [`DISPATCH_LOG`] (tests: CB2).
    dispatch_log: std::collections::VecDeque<(&'static str, Value)>,
}

/// How many dispatched actions [`App::dispatched`] keeps.
const DISPATCH_LOG: usize = 256;

/// Prepares an egui context for the shell: installs the bundled fonts and switches egui's own keyboard zoom off, so
/// that Ctrl+Plus, Ctrl+Minus and Ctrl+0 reach the canvas and never the UI scale (which stays `view.ui_scale`).
/// Call it before the first frame (eframe's creation closure, the test harness's context setup), so that the first
/// frame is laid out with them.
pub fn setup(ctx: &egui::Context) {
    crate::fonts::install(ctx);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

impl App {
    /// A frame over `shell` on `clock`; `initial` is a project to open after the first frame.
    pub fn new(
        shell: ShellModel,
        platform: Box<dyn Platform>,
        initial: Option<PathBuf>,
        clock: crate::clock::Clock,
    ) -> App {
        let dock = Dock::new(shell.layout(), shell.layout_version());
        App {
            shell,
            platform,
            clock,
            canvas: CanvasHost::new(),
            canvas_area: CanvasAreaHost::new(),
            navigator: NavigatorPane::new(),
            navigators: std::collections::BTreeMap::new(),
            properties: PropertiesPane::new(),
            find: FindPane::new(),
            toasts: crate::notifications::ToastHost::default(),
            job_started: std::collections::BTreeMap::new(),
            waker_set: false,
            area_splitters: 0,
            preview_view: PageView::new(),
            symbol_editor_view: crate::symbol_editor::EditorView::default(),
            dock,
            initial,
            // Built by the first frame from the shell's keymap (`App::sync_bindings`).
            bindings: Vec::new(),
            bindings_for: (ew_viewmodel::keymap::Preset::Ew, u64::MAX),
            title: String::new(),
            chooser_typing: crate::chooser::TypeAhead::default(),
            pending_pick: None,
            pending_prints: Vec::new(),
            applied_scale: None,
            window_zoom: None,
            applied_theme: None,
            request_paste: false,
            paste_in_place: false,
            ip_name: String::new(),
            drawing_fields_epoch: None,
            marker_csa: String::new(),
            marker_colours: String::new(),
            structure_caption: String::new(),
            structure_location: String::new(),
            text_buffer: None,
            command_bar: CommandBarHost::new(),
            page_pane: None,
            station_pane: None,
            hud_rect: None,
            options_rect: None,
            options_band: None,
            window_focused: None,
            focus_repaints: 0,
            new_project_checked: 0.0,
            disk_polled: (0.0, ew_viewmodel::ew_api::external::POLL_EVERY.as_secs_f64()),
            alt_alone: false,
            alt_down: false,
            dispatch_log: std::collections::VecDeque::new(),
        }
    }

    /// The actions the frame dispatched, oldest first (the last 256; tests: CB2).
    #[doc(hidden)]
    pub fn dispatched(&self) -> Vec<&'static str> {
        self.dispatch_log.iter().map(|(a, _)| *a).collect()
    }

    /// Dispatches `action` as a gesture of the window does, its failure reported as a banner.
    #[doc(hidden)]
    pub fn dispatch_from_window(&mut self, action: &'static str, params: &Value) {
        self.dispatch(action, params);
    }

    /// The actions the frame dispatched with their parameters, oldest first (the last 256).
    #[doc(hidden)]
    pub fn dispatched_calls(&self) -> Vec<(&'static str, Value)> {
        self.dispatch_log.iter().cloned().collect()
    }

    /// How many times the window regained the operating system's focus and requested its repaints (tests).
    #[doc(hidden)]
    pub fn focus_repaints(&self) -> u64 {
        self.focus_repaints
    }

    /// The command bar's and the HUD's rectangles in the last frame (tests: CB4).
    #[doc(hidden)]
    pub fn command_surface_rects(&self) -> (Option<egui::Rect>, Option<egui::Rect>, Option<egui::Rect>) {
        (self.command_bar.bar_rect(), self.hud_rect, self.page_pane)
    }

    /// The shell model.
    pub fn shell(&self) -> &ShellModel {
        &self.shell
    }

    /// The shell model, for the application binary's shutdown.
    pub fn shell_mut(&mut self) -> &mut ShellModel {
        &mut self.shell
    }

    /// Whether the canvas drew every visible tile of the page on show at the wanted resolution of a settled view.
    pub fn page_view_ready(&self) -> bool {
        self.canvas.is_ready()
    }

    /// How many times the page navigator built its rows (tests: a frame without a change builds none).
    #[doc(hidden)]
    pub fn navigator_rebuilds(&self) -> u64 {
        self.navigator.rebuilds()
    }

    /// The opacity each visible toast was painted with in the last frame, by serial (tests).
    pub fn toast_opacities(&self) -> Vec<(u64, f32)> {
        self.toasts.opacities()
    }

    /// How many times navigator `kind`'s pane built its rows, and how many rows its last frame painted (tests).
    pub fn navigator_pane_stats(&self, kind: ew_viewmodel::navigators::NavKind) -> Option<(u64, usize)> {
        match kind {
            ew_viewmodel::navigators::NavKind::Pages => {
                Some((self.navigator.rebuilds(), self.navigator.painted()))
            }
            k => self.navigators.get(&k).map(|p| (p.rebuilds(), p.painted())),
        }
    }

    /// The engine counters of the canvas (tests: tiles requested, rastered, uploaded; scenes built).
    pub fn canvas_counters(&self) -> CanvasCounters {
        self.canvas.counters()
    }

    /// The canvas area's group shares, left to right (tests: the splitter's position survives a tab click).
    #[doc(hidden)]
    pub fn canvas_area_shares(&self) -> Option<Vec<f32>> {
        self.canvas_area.shares()
    }

    /// The number of tile textures the canvas holds (tests).
    pub fn canvas_cached_tiles(&self) -> usize {
        self.canvas.cached_tiles()
    }

    /// The frame statistics of the canvas (section 9).
    pub fn canvas_stats(&self) -> &ew_render2d::stats::FrameStats {
        self.canvas.stats()
    }

    /// Gives the canvas eframe's wgpu render state (`None`: the CPU canvas) and the command line's flags; the
    /// application binary calls it from eframe's creation closure (section 7.2).
    pub fn attach_gpu(
        &mut self,
        render_state: Option<egui_wgpu::RenderState>,
        flags: ew_render2d::gpu::Flags,
    ) {
        self.canvas.gpu_mut().attach(render_state, flags);
        self.canvas.sync_followers();
    }

    /// The GPU side of the canvas (tests: stand in for a device, inject a device loss).
    #[doc(hidden)]
    pub fn gpu_host_mut(&mut self) -> &mut GpuHost {
        self.canvas.gpu_mut()
    }

    /// Whether the image of the wizard's preview has arrived from the render worker.
    pub fn preview_ready(&self) -> bool {
        self.preview_view.is_ready()
    }

    /// The number of splitter handles on screen: the dock's and the canvas area's while the project view shows,
    /// else none (tests; every handle is a named `Splitter` node).
    #[doc(hidden)]
    pub fn splitter_count(&self) -> usize {
        if !matches!(self.shell.screen(), Screen::FirstRun | Screen::Wizard) && self.shell.has_project() {
            self.dock.splitters() + self.area_splitters
        } else {
            0
        }
    }

    /// The bytes of cached tiles of scenes not drawn in the last frame (for tests).
    #[doc(hidden)]
    pub fn canvas_hidden_bytes(&self) -> u64 {
        self.canvas.hidden_bytes()
    }

    /// The number of tile scenes the canvases keep (tests).
    #[doc(hidden)]
    pub fn canvas_scene_count(&self) -> usize {
        self.canvas.scene_count()
    }

    /// The preview chips painted in the last frame with their frames in points, and the statistics panel's frame
    /// when it showed (tests).
    #[doc(hidden)]
    pub fn preview_chips(&self) -> (Vec<(String, egui::Rect)>, Option<egui::Rect>) {
        let (chips, stats) = self.canvas.preview_chips();
        (chips.to_vec(), stats)
    }

    /// The canvas host (tests; the chips of the last frame, the chrome-obstacle seam).
    #[doc(hidden)]
    pub fn canvas_host(&mut self) -> &mut CanvasHost {
        &mut self.canvas
    }

    /// The panes of the dock tree in layout order (tests).
    pub fn dock_panes(&self) -> Vec<PaneKind> {
        self.dock.panes()
    }

    /// The active pane of the tab group that holds `pane` (tests).
    #[doc(hidden)]
    pub fn dock_active_beside(&self, pane: PaneKind) -> Option<PaneKind> {
        self.dock.active_beside(pane)
    }

    /// Puts `pane` into the tab group of `target`, as a drag of its title onto the other pane does (tests).
    #[doc(hidden)]
    pub fn dock_tab_together(&mut self, pane: PaneKind, target: PaneKind) {
        self.dock.tab_together(pane, target);
    }

    fn dispatch(&mut self, action: &'static str, params: &Value) {
        // A paste without the clipboard's text (Edit ▸ Paste, the palette, a Ctrl+V key press that
        // the platform did not turn into a paste event) asks the platform for the system clipboard first; an action
        // the shell refuses now is dispatched, so its reason shows.
        // The table's paste likewise.
        if action == "table.paste"
            && params.get("text").is_none()
            && ew_viewmodel::actions::find(action).is_some_and(|a| a.enabled_now(&self.shell).is_ok())
        {
            self.request_paste = true;
            return;
        }
        if matches!(action, "edit.paste_clip" | "edit.paste_in_place")
            && params.get("text").is_none()
            && self.shell.paste_page_reason().is_none()
            && ew_viewmodel::actions::find(action).is_some_and(|a| a.enabled_now(&self.shell).is_ok())
        {
            self.request_paste = true;
            self.paste_in_place = action == "edit.paste_in_place";
            return;
        }
        if self.dispatch_log.len() == DISPATCH_LOG {
            self.dispatch_log.pop_front();
        }
        self.dispatch_log.push_back((action, params.clone()));
        self.run(Some(action), |shell| shell.dispatch(action, params));
    }

    /// Runs `step` on the shell, which says its outcome where the person looks ([`ShellModel::run_said`]).
    fn run(&mut self, action: Option<&str>, step: impl FnOnce(&mut ShellModel) -> Outcome) {
        // The window reads the date before an action; the shell never does (the PLC export's form starts with it).
        self.shell.set_today(self.clock.today());
        let properties = &self.properties;
        self.shell.run_said(action, step, |row| properties.drew_row(row));
    }
}
