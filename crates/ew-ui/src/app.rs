// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui frame over the shell model (WP-S2.01 parts 1 and 2, spec section 5.3): the command bar with the app menu
//! (WP-S2.17 part 2), the banner, the start page, the first-run screen, the wizard, or the docked project view (the
//! page list and the page canvas of WP-S2.02, with the tool HUD and the zoom control over it), the status bar, the
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
    /// A row of the palette's parameter form changed (WP-S2.08 D5 (a)): its index and its text.
    PaletteField(usize, String),
    /// An answer to the unsaved-changes dialog.
    Confirm(ConfirmAnswer),
    /// The go-to field changed.
    GotoText(String),
    /// Go in the go-to dialog: a folio or a cell.
    GotoGo(String),
    /// The save-layout field changed.
    WorkspaceText(String),
    /// A field of a creation dialog changed (FT-408, FT-409): the parameter and its text.
    FormField(&'static str, String),
    /// Enter in a creation dialog, or its Create button.
    FormSubmit,
    /// A cell of the form's table changed (FT-786): the row, the column (`None`: the first, a rename) and its text.
    FormTableCell(usize, Option<usize>, String),
    /// The form table's Add (FT-786).
    FormTableAdd,
    /// The form table's remove button of a row (FT-786).
    FormTableRemove(usize),
    /// The banner's own button.
    DismissBanner,
    /// The privacy button of the About dialog and of the first-run privacy step: `PRIVACY.md` in the system's viewer
    /// (WP-S6.05 D16: the bundled copy when there is one).
    OpenPrivacy,
    /// The junction sequence editor's focused row (WP-S2.04 part 2; a view change of the popup).
    JunctionRow(usize),
    /// A toast's Close (WP-S2.09 §6.5): the toast leaves the screen, the log keeps it.
    DismissToast(u64),
    /// A click on a disabled command (FT-1025): its reason in words, as a toast.
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
    /// From the dialog's own thread (FT-467): the frame reads the receiver at the start of every frame, and the thread
    /// requests a repaint once the answer is in it ([`spawn_pick`]).
    Later(Receiver<Pick>),
}

/// Runs `ask` (a native file dialog) on a thread of its own and returns the receiver of its answer; the thread sends
/// the answer and then requests a repaint of `ctx`, so the frame that the request starts finds it (FT-467).
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
    /// its event loop while a dialog is open (FT-467). `ctx` is the context to repaint when the answer is there.
    fn start_pick(&mut self, prompt: Prompt, ctx: &egui::Context) -> PickReply {
        let _ = ctx;
        PickReply::Now(self.pick(prompt))
    }

    /// Opens a URL in the system browser: a `file://` URL of the installed user guide, or the repository's
    /// `PRIVACY.md` (a user action; the application itself sends nothing); `false` when it could not be opened.
    fn open_url(&mut self, _url: &str) -> bool {
        false
    }

    /// Prints a built print set (WP-S3.07 part 3, ruling 3 of ID-1180): `request` holds the PDF, the title and
    /// whether Print… or Open in PDF viewer asked. The application binary prints with `ew-print` on a worker thread
    /// and answers later; `ctx` is the context to repaint when the end is there. The default refuses: printing is not
    /// available in this window.
    fn print(&mut self, request: PrintRequest, ctx: &egui::Context) -> PrintReply {
        let _ = (request, ctx);
        PrintReply::Now(PrintEnd::Unavailable)
    }
}

/// How the end of a print comes to the frame (WP-S3.07 part 3).
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

/// How long a toast stays on screen (WP-S2.09 D6), measured by the window's frame clock.
const TOAST_SECONDS: f64 = 8.0;

/// The opacity of the tool options card while the tool's ghost is under it (FT-1086, doc 16a §5.5).
const OPTIONS_FADED_OPACITY: f32 = 0.4;

/// A job younger than this shows no status-bar field (WP-S2.09 §6.4).
const JOB_FIELD_AFTER: Duration = Duration::from_millis(200);

/// The application frame: the shell model and what the window needs besides it.
pub struct App {
    shell: ShellModel,
    platform: Box<dyn Platform>,
    /// The page canvases of the project view (WP-S2.02, WP-S2.03).
    canvas: CanvasHost,
    /// The canvas area's nested tab tree (WP-S2.03).
    canvas_area: CanvasAreaHost,
    /// The page navigator pane (WP-S2.03).
    navigator: NavigatorPane,
    // WP-S2.06 part 1
    /// The panes of the other open navigators, by kind.
    navigators: std::collections::BTreeMap<ew_viewmodel::navigators::NavKind, NavigatorPane>,
    /// The property grid pane (WP-S2.05).
    properties: PropertiesPane,
    /// The banner text [`App::report`] raised for a failure, while it stands: the next failure replaces it, where a
    /// banner the shell raised itself keeps precedence (W46.4 F1); with the action that failed, whose next success
    /// clears it (ID-1278 ruling 8).
    reported: Option<String>,
    /// The action whose failure raised [`App::reported`]: the open palette form's when the form ran it.
    reported_by: Option<String>,
    /// The find pane (WP-S2.05 part 2).
    find: FindPane,
    /// The visible toasts by serial and the frame time (egui's `input.time`, seconds) of the frame that first painted
    /// each: a toast goes after 8 seconds measured by the window, never by the model (WP-S2.09 D6). The frame clock is
    /// the wall clock in the window and the harness's steps in a test, so no test depends on how fast a loaded machine
    /// runs (FT-524).
    toast_since: std::collections::BTreeMap<u64, f64>,
    /// FT-1027: the opacity each visible toast was painted with in the last frame (its fade in and out on the frame
    /// clock), and the frame time of that frame (a hovered toast's clock stands still between two frames).
    toast_opacity: std::collections::BTreeMap<u64, f32>,
    toast_clock: Option<f64>,
    /// The toasts the pointer was on in the last frame (their clocks stand still).
    toast_hovered: Vec<u64>,
    /// The running jobs by number and the frame time (egui's `input.time`, seconds) the window first saw each (the
    /// elapsed time and the 200 ms rule, §6.4; FT-570: the frame clock, like the toasts').
    job_started: std::collections::BTreeMap<u64, f64>,
    /// Whether the shell has the window's repaint request as its jobs' waker.
    waker_set: bool,
    /// The canvas area's splitter handles on screen in the last frame.
    area_splitters: usize,
    /// The page view of the wizard's preview.
    preview_view: PageView,
    dock: Dock,
    initial: Option<PathBuf>,
    bindings: Vec<Binding>,
    /// The keymap the bindings were built for (WP-S2.08 part 2): the preset and the user keymap's version.
    bindings_for: (ew_viewmodel::keymap::Preset, u64),
    title: String,
    palette_was_open: bool,
    /// Whether the picker was open in the previous frame (its filter field takes the focus when it opens).
    picker_was_open: bool,
    /// The Join chooser's type-ahead text (WP-S2.18 D2).
    chooser_typing: crate::chooser::TypeAhead,
    /// The opening of the dialog the last frame showed ([`ShellModel::dialog_generation`]): a dialog opened since is a
    /// new one, which takes its first focus (doc 16a §6.12), even when it replaced the last in one frame (W49.3-F1: the
    /// collision dialog after Renumber) or is of the same kind (C49-p10b#4: rebuilt by a second change).
    dialog_was: Option<u64>,
    /// The file dialog that is open off the window's thread, with the request it answers (FT-467).
    pending_pick: Option<(PromptRequest, Receiver<Pick>)>,
    /// The prints running off the window's thread (WP-S3.07 part 3), each with the receiver of its end.
    pending_prints: Vec<Receiver<PrintEnd>>,
    /// The `ui_scale` last given to egui: the zoom is set when the setting changes, not every frame, so that
    /// egui's own zoom keys keep working in between.
    applied_scale: Option<f32>,
    /// The zoom factor of the previous frame, for the window memory (its report of a frame uses that zoom).
    window_zoom: Option<f32>,
    /// The theme, density, minimum text size and reduce-motion settings last given to egui.
    applied_theme: Option<(String, String, u8, bool)>,
    // WP-S2.05 part 3
    /// Edit ▸ Paste (or the palette) asked for the system clipboard: the frame sends `ViewportCommand::RequestPaste`,
    /// and the platform answers with a paste event, which runs `edit.paste_clip` with its text like Ctrl+V.
    request_paste: bool,
    /// The IP tool's name field in the tool strip (WP-S2.04 §9.1): `ip.name` on Enter.
    ip_name: String,
    /// The Wire marker tool's cross-section and colours fields (WP-S2.04 §10.5): `marker.fields` on Enter.
    marker_csa: String,
    marker_colours: String,
    /// The Structure box tool's caption and location fields (WP-S2.04 §10.5): `structure_box.fields` on Enter.
    structure_caption: String,
    structure_location: String,
    /// The tool generation whose drawing-field drafts are displayed.
    drawing_fields_epoch: Option<u64>,
    /// The text popup's field (WP-S2.04 §10.5): `None` while the popup is closed.
    text_buffer: Option<String>,
    // WP-S2.17 part 2
    /// The command bar's window state (the collapsed bar's popover, the search field's place).
    command_bar: CommandBarHost,
    /// The page view pane's rectangle in the last frame: the HUD, the zoom control and the tool options stand on it.
    page_pane: Option<egui::Rect>,
    /// The HUD's rectangle in the last frame.
    hud_rect: Option<egui::Rect>,
    /// The tool options card's rectangle in the last frame (FT-1086: where the ghost is under it).
    options_rect: Option<egui::Rect>,
    /// Whether the window had the operating system's focus in the last frame (FT-514).
    window_focused: Option<bool>,
    /// How many times the window regained the focus and asked for the frames that repaint it (FT-514; tests).
    focus_repaints: u64,
    /// When the project on disk was last polled for an external change (WP-S1.17 D20), on the window's clock, and the
    /// time to the next poll in seconds.
    disk_polled: (f64, f64),
    /// Alt is down and nothing else was pressed since: its release opens the app menu (doc 16a §5.2).
    alt_alone: bool,
    /// Whether Alt was down in the last frame.
    alt_down: bool,
    /// Whether the shell's focus was on the command bar in the last frame (F6 arrives: the active tab takes egui's
    /// focus).
    bar_had_focus: bool,
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
    /// A frame over `shell`; `initial` is a project to open after the first frame.
    pub fn new(shell: ShellModel, platform: Box<dyn Platform>, initial: Option<PathBuf>) -> App {
        let dock = Dock::new(shell.layout(), shell.layout_version());
        App {
            shell,
            platform,
            canvas: CanvasHost::new(),
            canvas_area: CanvasAreaHost::new(),
            navigator: NavigatorPane::new(),
            navigators: std::collections::BTreeMap::new(),
            properties: PropertiesPane::new(),
            reported: None,
            reported_by: None,
            find: FindPane::new(),
            toast_since: std::collections::BTreeMap::new(),
            toast_opacity: std::collections::BTreeMap::new(),
            toast_clock: None,
            toast_hovered: Vec::new(),
            job_started: std::collections::BTreeMap::new(),
            waker_set: false,
            area_splitters: 0,
            preview_view: PageView::new(),
            dock,
            initial,
            // Built by the first frame from the shell's keymap (`App::sync_bindings`).
            bindings: Vec::new(),
            bindings_for: (ew_viewmodel::keymap::Preset::Ew, u64::MAX),
            title: String::new(),
            palette_was_open: false,
            picker_was_open: false,
            chooser_typing: crate::chooser::TypeAhead::default(),
            dialog_was: None,
            pending_pick: None,
            pending_prints: Vec::new(),
            applied_scale: None,
            window_zoom: None,
            applied_theme: None,
            request_paste: false,
            ip_name: String::new(),
            drawing_fields_epoch: None,
            marker_csa: String::new(),
            marker_colours: String::new(),
            structure_caption: String::new(),
            structure_location: String::new(),
            text_buffer: None,
            command_bar: CommandBarHost::new(),
            page_pane: None,
            hud_rect: None,
            options_rect: None,
            window_focused: None,
            focus_repaints: 0,
            disk_polled: (0.0, ew_viewmodel::ew_api::external::POLL_EVERY.as_secs_f64()),
            alt_alone: false,
            alt_down: false,
            bar_had_focus: false,
            dispatch_log: std::collections::VecDeque::new(),
        }
    }

    /// The actions the frame dispatched, oldest first (the last 256; tests: CB2).
    #[doc(hidden)]
    pub fn dispatched(&self) -> Vec<&'static str> {
        self.dispatch_log.iter().map(|(a, _)| *a).collect()
    }

    /// Dispatches `action` as a gesture of the window does, its failure reported as a banner (tests: W46.4 F1).
    #[doc(hidden)]
    pub fn dispatch_from_window(&mut self, action: &'static str, params: &Value) {
        self.dispatch(action, params);
    }

    /// The actions the frame dispatched with their parameters, oldest first (the last 256; tests: DK9).
    #[doc(hidden)]
    pub fn dispatched_calls(&self) -> Vec<(&'static str, Value)> {
        self.dispatch_log.iter().cloned().collect()
    }

    /// How many times the window regained the operating system's focus and requested its repaints (FT-514; tests).
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

    /// The opacity each visible toast was painted with in the last frame, by serial (FT-1027; tests).
    pub fn toast_opacities(&self) -> Vec<(u64, f32)> {
        self.toast_opacity.iter().map(|(s, a)| (*s, *a)).collect()
    }

    // WP-S2.06 part 1
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

    /// The frame statistics of the canvas (WP-S2.02 section 9).
    pub fn canvas_stats(&self) -> &ew_render2d::stats::FrameStats {
        self.canvas.stats()
    }

    /// Gives the canvas eframe's wgpu render state (`None`: the CPU canvas) and the command line's flags; the
    /// application binary calls it from eframe's creation closure (WP-S2.02 section 7.2).
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
    /// else none (tests; FT-338: every handle is a named `Splitter` node).
    #[doc(hidden)]
    pub fn splitter_count(&self) -> usize {
        if self.shell.first_run().is_none() && self.shell.wizard().is_none() && self.shell.has_project() {
            self.dock.splitters() + self.area_splitters
        } else {
            0
        }
    }

    /// The bytes of cached tiles of scenes not drawn in the last frame (tests: FT-356's share).
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
    /// when it showed (tests, FT-389).
    #[doc(hidden)]
    pub fn preview_chips(&self) -> (Vec<(String, egui::Rect)>, Option<egui::Rect>) {
        let (chips, stats) = self.canvas.preview_chips();
        (chips.to_vec(), stats)
    }

    /// The canvas host (tests; WP-S2.17 part 5: the chips of the last frame, the chrome-obstacle seam).
    #[doc(hidden)]
    pub fn canvas_host(&mut self) -> &mut CanvasHost {
        &mut self.canvas
    }

    /// The panes of the dock tree in layout order (tests).
    pub fn dock_panes(&self) -> Vec<PaneKind> {
        self.dock.panes()
    }

    /// The active pane of the tab group that holds `pane` (tests, FT-509).
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
        // WP-S2.05 part 3: a paste without the clipboard's text (Edit ▸ Paste, the palette, a Ctrl+V key press that
        // the platform did not turn into a paste event) asks the platform for the system clipboard first; an action
        // the shell refuses now is dispatched, so its reason shows.
        // WP-S2.06 part 3: the table's paste likewise.
        if action == "table.paste"
            && params.get("text").is_none()
            && ew_viewmodel::actions::find(action).is_some_and(|a| (a.enabled)(&self.shell).is_ok())
        {
            self.request_paste = true;
            return;
        }
        if action == "edit.paste_clip"
            && params.get("text").is_none()
            && self.shell.paste_page_reason().is_none()
            && ew_viewmodel::actions::find(action).is_some_and(|a| (a.enabled)(&self.shell).is_ok())
        {
            self.request_paste = true;
            return;
        }
        if self.dispatch_log.len() == DISPATCH_LOG {
            self.dispatch_log.pop_front();
        }
        self.dispatch_log.push_back((action, params.clone()));
        self.run(Some(action), |shell| shell.dispatch(action, params));
    }

    /// Runs `step` on the shell and reports its outcome. A banner that changed across the step was the shell's own
    /// (raised, replaced or cleared for its own reason): the banner [`App::report`] raised is then forgotten.
    fn run(&mut self, action: Option<&str>, step: impl FnOnce(&mut ShellModel) -> Outcome) {
        let before = self.shell.banner().map(str::to_owned);
        // The palette form's run is reported as its action (ID-1278 ruling 8), and (FT-1260) the palette's run of
        // its selected action without a form as that action.
        let form = self.shell.palette().and_then(|p| match &p.form {
            Some(f) => Some(f.action),
            None => match self.shell.palette_entries().get(p.selected)?.run {
                ew_viewmodel::actions::PaletteRun::Action(id) => Some(id),
                _ => None,
            },
        });
        let outcome = step(&mut self.shell);
        if self.shell.banner() != before.as_deref() {
            self.reported = None;
        }
        self.report(outcome, action.or(form));
    }

    /// A failure or incomplete Cut refusal becomes a banner when the shell shows none of its own; a banner this raised
    /// before is replaced by the new failure (W46.4 F1: a second refusal kept the first's banner, which named the wrong
    /// point). One fact in one place (doc 16a P3): a refusal the navigator's inline field shows under itself (an F2
    /// tag's, FT-633; cycle 37, window check W37) or that the Properties pane shows under a drawn row (`action` a
    /// `props.*` one whose dispatch set the row's error; W46.4 F1) is not made a banner as well.
    ///
    /// ID-1278 ruling 8: a failure the open palette form shows under its fields is not made a banner either, and the
    /// banner this raised for an action's failure clears when that action next succeeds.
    fn report(&mut self, outcome: Outcome, action: Option<&str>) {
        if self.shell.nav_rename_error().is_some() {
            return;
        }
        let ours = self.reported.is_some() && self.shell.banner() == self.reported.as_deref();
        let message = match outcome {
            Outcome::Failed { message } => message,
            Outcome::Refused { reason_key } if reason_key == key!("reason-cut-incomplete-items") => {
                strings::text(reason_key).to_owned()
            }
            // FT-1260: an action that opened its parameter form has not run: its banner stays.
            Outcome::Done
                if ours
                    && action.is_some()
                    && self.reported_by.as_deref() == action
                    && !self
                        .shell
                        .palette()
                        .and_then(|p| p.form.as_ref())
                        .is_some_and(|f| Some(f.action) == action) =>
            {
                self.shell.dismiss_banner();
                self.reported = None;
                self.reported_by = None;
                return;
            }
            _ => return,
        };
        let in_form = self
            .shell
            .palette()
            .and_then(|p| p.form.as_ref())
            .is_some_and(|f| f.error.as_deref() == Some(message.as_str()));
        if in_form {
            return;
        }
        let under_row = action.is_some_and(|a| a.starts_with("props."))
            && self
                .shell
                .properties()
                .error()
                // The pane shows the message with a capital first (FT-1075), so it is that form that is the same.
                .is_some_and(|(row, shown)| {
                    shown == strings::sentence_start(&message) && self.properties.drew_row(row)
                });
        if under_row {
            return;
        }
        // A state banner (W49.3-F2) lies under a message: the message shows over it.
        if self.shell.message_banner().is_none() || ours {
            self.shell.show_banner(&message);
            self.reported = Some(message);
            self.reported_by = action.map(str::to_owned);
        }
    }

    /// The banner of doc 16a §6.11 (WP-S2.17 part 4) as an overlay (FT-822): along the top edge of the canvas below the
    /// page tabs when a page view is shown, else under the command bar, full width of that area. Showing or dismissing
    /// it never moves the canvas or a panel (the play-test's clicks missed when it pushed the window down). Returns its
    /// rectangle.
    fn banner_overlay(
        &self,
        ctx: &egui::Context,
        tokens: &crate::theme::Tokens,
        bar_bottom: f32,
        events: &mut Vec<Event>,
    ) -> Option<egui::Rect> {
        let banner = self.shell.banner()?;
        let area = match self.page_pane {
            Some(pane) => egui::Rect::from_min_max(
                egui::pos2(
                    pane.left(),
                    (pane.top() + tokens.size_dock_tab_bar).min(pane.bottom()),
                ),
                pane.max,
            ),
            None => {
                let content = ctx.content_rect();
                egui::Rect::from_min_max(egui::pos2(content.left(), bar_bottom), content.max)
            }
        };
        let frame = egui::Frame::NONE
            .fill(tokens.warning_subtle.color32())
            .inner_margin(egui::Margin::symmetric(
                crate::theme::margin_px(tokens.space_4),
                crate::theme::margin_px(tokens.space_2),
            ));
        let inner_width = (area.width() - 2.0 * tokens.space_4).max(0.0);
        let response = egui::Area::new(egui::Id::unique("banner"))
            .fixed_pos(area.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.set_clip_rect(area);
                frame.show(ui, |ui| {
                    ui.set_width(inner_width);
                    ui.horizontal_wrapped(|ui| {
                        ui.set_min_height(tokens.size_control);
                        let (r, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(tokens.icon_md), egui::Sense::hover());
                        crate::icons::paint(
                            ui.painter(),
                            crate::icons::IconId::alert_triangle,
                            r,
                            tokens.warning_icon.color32(),
                            tokens.icon_stroke,
                        );
                        ui.label(egui::RichText::new(banner).color(tokens.text_primary.color32()));
                        if self.shell.can_discard_journal()
                            && crate::components::button(
                                ui,
                                crate::components::Kind::Primary,
                                strings::text(key!("action-file-discard-journal")),
                                true,
                                0.0,
                            )
                            .clicked()
                        {
                            events.push(Event::Action("file.discard_journal", Value::Null));
                        }
                        // W49.3-F2 (doc 16a §6.11): the held-saves banner's one Primary reopens the choice; a message over
                        // it (a refused save) keeps it (W49.3b-N1).
                        if self.shell.external_choice_waiting()
                            && crate::components::button(
                                ui,
                                crate::components::Kind::Primary,
                                strings::text(key!("banner-external-choose")),
                                true,
                                0.0,
                            )
                            .clicked()
                        {
                            events.push(Event::Action("external.review", Value::Null));
                        }
                        if self.shell.banner_dismissable()
                            && crate::components::button(
                                ui,
                                crate::components::Kind::Subtle,
                                strings::text(key!("banner-dismiss")),
                                true,
                                0.0,
                            )
                            .clicked()
                        {
                            events.push(Event::DismissBanner);
                        }
                    });
                });
            });
        Some(response.response.rect)
    }

    /// Paints one frame into the root `ui` of the window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let _frame = tracing::trace_span!("frame").entered();
        let ctx = ui.ctx().clone();
        // `ui_scale` is a factor on the native scale: egui's zoom factor is exactly that, and follows the window
        // to another monitor. It is set once when the setting changes.
        // WP-S2.08 part 2: the bindings follow the keymap preset and the user's keymap.
        let keymap = (self.shell.keymap_preset(), self.shell.keymap_version());
        if self.bindings_for != keymap {
            self.bindings = keys::bindings(&self.shell.bindings());
            self.bindings_for = keymap;
        }
        let scale = self.shell.settings().ui_scale;
        if self.applied_scale != Some(scale) {
            ctx.set_zoom_factor(scale);
            self.applied_scale = Some(scale);
        }
        // WP-S2.17 part 1: the theme and the density as tokens (doc 16a §4.14), applied when either setting changes;
        // `system` lets egui follow the operating system between Light and Dark (D5).
        // WP-S2.08 D14: the minimum text size and reduced motion with them.
        let appearance = (
            self.shell.settings().theme.clone(),
            self.shell.settings().density.clone(),
            self.shell.settings().min_font_pt,
            self.shell.reduce_motion(),
        );
        if self.applied_theme.as_ref() != Some(&appearance) {
            crate::theme::apply_setting(&appearance.0, &appearance.1, appearance.2, appearance.3, &ctx);
            // A visual run draws the caret always (WP-S6.13 part 4 §3.1), so a frame never catches it off.
            if crate::clock::on_frame_clock() {
                ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
            }
            self.applied_theme = Some(appearance);
        }
        // FT-1247: reduced motion ends egui's wheel smoothing too.
        if self.shell.reduce_motion() {
            crate::theme::unsmoothed_scroll(&ctx);
        }
        // The switch rule (WP-S2.02 section 7.2): the setting is effective at once, a failure moves the canvas to the
        // CPU with one banner, and the status bar shows the result.
        self.canvas.update_backend(&mut self.shell);
        // WP-S2.17 part 3 (DK6, FT-348): the window's geometry, written at the end of the window.
        crate::window_memory::remember(&ctx, &mut self.shell, &mut self.window_zoom);
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.should_quit() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            // While a file dialog is open nothing the user does reaches the shell (see below): the window stays open.
            if self.pending_pick.is_none() {
                self.dispatch("app.quit", &Value::Null);
            }
        }
        // FT-514: a native dialog closed, or the user came back from another window: the window regains the focus
        // without a pointer move, and the status hint it shows may be stale. The frame that sees the focus come back
        // repaints now and twice shortly after (a dialog may still be closing while the first frame paints).
        let focused = ctx.input(|i| i.viewport().focused);
        let regained = focused == Some(true) && self.window_focused == Some(false);
        if focused == Some(true) && self.window_focused == Some(false) {
            ctx.request_repaint();
            ctx.request_repaint_after(Duration::from_millis(100));
            ctx.request_repaint_after(Duration::from_millis(500));
            self.focus_repaints += 1;
        }
        if focused.is_some() {
            self.window_focused = focused;
        }
        // WP-S2.17 part 2: Alt pressed and released alone, or F10, opens the app menu (doc 16a §5.2).
        self.app_menu_keys(&ctx);
        // WP-S2.09 part 1: the jobs wake the window after each progress report and at their end, and every frame takes
        // the ends that are in (no `Session` call).
        // On the frame clock (a visual run) the driver waits for the jobs itself: no job thread wakes the frame, so
        // which frame sees an end never depends on timing.
        if !self.waker_set && !crate::clock::on_frame_clock() {
            let waker = ctx.clone();
            self.shell
                .set_job_waker(Some(std::sync::Arc::new(move || waker.request_repaint())));
            self.waker_set = true;
        }
        self.shell.poll_jobs();
        let now = ctx.input(|i| i.time);
        // WP-S1.17 part 7 (D20, D22): the project on disk is polled every 3 s, 750 ms later again while a change settles,
        // and when the window regains the focus; never while a pointer button is held (a gesture) or on the frame clock
        // (a visual run polls through `project.check_disk`).
        if self.shell.has_project() && !crate::clock::on_frame_clock() {
            let (last, every) = self.disk_polled;
            let gesture = ctx.input(|i| i.pointer.any_down());
            if (regained || now - last >= every) && !gesture {
                let settling = self.shell.poll_disk();
                let next = if settling {
                    ew_viewmodel::ew_api::external::QUIESCENCE
                } else {
                    ew_viewmodel::ew_api::external::POLL_EVERY
                };
                self.disk_polled = (now, next.as_secs_f64());
            }
            ctx.request_repaint_after(Duration::from_secs_f64(self.disk_polled.1));
        }
        let running: Vec<u64> = self
            .shell
            .jobs()
            .entries()
            .filter(|e| e.is_running())
            .map(|e| e.number)
            .collect();
        self.job_started.retain(|n, _| running.contains(n));
        for n in &running {
            self.job_started.entry(*n).or_insert(now);
        }
        // The 200 ms rule: a job that ends sooner shows no status-bar field; one that runs on shows it from then.
        let hide_job = self
            .shell
            .jobs()
            .newest_running()
            .and_then(|e| self.job_started.get(&e.number))
            .is_some_and(|t| now - *t < JOB_FIELD_AFTER.as_secs_f64());
        if !running.is_empty() {
            ctx.request_repaint_after(if hide_job {
                JOB_FIELD_AFTER
            } else {
                Duration::from_millis(500)
            });
        }
        if let Some(dir) = self.initial.take() {
            self.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() }));
        }
        // The file dialogs are started and answered at the start of a frame, so the menu is already closed and the card
        // is in the shell before this frame paints (W3); an answer from a dialog's thread starts this frame (FT-467).
        self.answer_prompts(&ctx);
        if self.dock.version() != self.shell.layout_version() {
            self.dock = Dock::new(self.shell.layout(), self.shell.layout_version());
        }
        // The preview is built from the draft before the frame reads it (a no-op while it is current).
        self.shell.wizard_prepare_preview();

        let mut events: Vec<Event> = Vec::new();
        self.properties.begin_frame();
        // ID-1276 ruling 2: Ctrl+Enter submits a form dialog from any focus. The form takes the key before the keymap
        // and the panes under it, so their Ctrl+Enter (`canvas.activate_alt`, `preview.apply`, `find.replace_all`)
        // does not fire.
        let form_key = matches!(self.shell.dialog(), Some(ew_viewmodel::shell::Dialog::Form(_)))
            && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        self.route_keys(&ctx, &mut events);
        // Cycle 38 (window check W38): while the picker is open it owns the keyboard. The panes paint before it and
        // a focused one (the navigator's type-ahead, a Properties row, the canvas) would take the frame's keys and
        // text first: the key typed in the frame that opens the picker went to the focused navigator ("elay" for
        // "relay"). The keyboard events are held back from the panes and handed to the picker when it paints.
        let held_for_picker: Vec<egui::Event> = if self.shell.picker().is_some() {
            ctx.input_mut(|i| {
                let (held, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut i.events)
                    .into_iter()
                    .partition(is_keyboard_event);
                i.events = rest;
                held
            })
        } else {
            Vec::new()
        };
        // A file from the OS dropped on the window (doc 16 §4.8): an image becomes the Image tool's asset.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if let Some(file) = dropped_image(&dropped) {
            events.push(Event::Action(
                "image.file",
                json!({ "path": file.to_string_lossy() }),
            ));
        }

        // The chrome paints `color.surface.chrome` itself (doc 16a §4.14); panels take `color.surface.panel`.
        let tokens = crate::theme::current(&ctx);
        let chrome = tokens.surface_chrome.color32();
        // Doc 16a §5.5: a fit keeps the sheet above the tool HUD's band.
        self.shell
            .set_canvas_fit_inset(f64::from(crate::hud::fit_inset(&tokens)));
        // WP-S2.17 part 2: the command bar replaces the menu bar (doc 16a §5.2); the app button opens the app menu.
        let bar_height = crate::command_bar::bar_height(&tokens, self.shell.command_bar_collapsed());
        let command_bar = &mut self.command_bar;
        let shell = &self.shell;
        Panel::top("command-bar")
            .exact_size(bar_height)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(chrome))
            .show(ui, |ui| command_bar.show(ui, shell, &mut events));
        // F6 reached the command bar: its active tab takes egui's keyboard focus.
        let bar_has_focus = self.shell.focus() == Some(Focus::CommandBar);
        if bar_has_focus && !self.bar_had_focus {
            let tab = crate::command_bar::tab_id(self.shell.command_tab());
            ctx.memory_mut(|m| m.request_focus(tab));
        }
        self.bar_had_focus = bar_has_focus;
        // WP-S2.17 part 3: `size.status` high (doc 16a §5.8).
        Panel::bottom("status-bar")
            .exact_size(tokens.size_status)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(chrome))
            .show(ui, |ui| status_bar::show(ui, &self.shell, hide_job, &mut events));
        let tool_epoch = self.shell.tool_epoch();
        if self.drawing_fields_epoch != Some(tool_epoch) {
            self.marker_csa.clear();
            self.marker_colours.clear();
            self.structure_caption.clear();
            self.structure_location.clear();
            self.drawing_fields_epoch = Some(tool_epoch);
        }
        let mut new_layout = None;
        let mut canvas_shown = false;
        let project_view =
            self.shell.first_run().is_none() && self.shell.wizard().is_none() && self.shell.has_project();
        if project_view {
            self.canvas.begin_frame(&ctx, &self.shell);
        }
        self.area_splitters = 0;
        if project_view {
            // WP-S2.17 part 3 (doc 16a §5.4, §5.6): the navigator rail at the left, the palettes rail at the right.
            for (side, left) in [("rail-left", true), ("rail-right", false)] {
                let panel = if left {
                    Panel::left(side)
                } else {
                    Panel::right(side)
                };
                panel
                    .exact_size(tokens.size_rail)
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(egui::Frame::NONE.fill(chrome))
                    .show(ui, |ui| {
                        if left {
                            crate::rail::show_left(ui, &self.shell, &mut events);
                        } else {
                            crate::rail::show_right(ui, &self.shell, &mut events);
                        }
                    });
            }
        }
        let mut page_pane = None;
        CentralPanel::default().show(ui, |ui| {
            crate::properties::remember_free_area(ui);
            if self.shell.first_run().is_some() {
                first_run::show(ui, &self.shell, &mut events);
            } else if self.shell.wizard().is_some() {
                wizard::show(ui, &self.shell, &mut self.preview_view, &mut events);
            } else if self.shell.has_project() {
                let App {
                    shell,
                    dock,
                    canvas,
                    canvas_area,
                    navigator,
                    navigators,
                    properties,
                    find,
                    area_splitters,
                    job_started,
                    ..
                } = self;
                navigators.retain(|k, _| shell.navigator_open(*k));
                let collapsed = shell.rail_collapsed();
                // Doc 16a §5.7: the Messages tab's count chip shows the open findings (FT-710: the status bar's rule).
                let counts = [(
                    PaneKind::Navigator(ew_viewmodel::navigators::NavKind::Messages),
                    shell.messages_badge(),
                )];
                let headers = dock.show(ui, collapsed, &counts, &mut |ui, pane| match pane {
                    PaneKind::Pages => navigator.show(ui, shell, &mut events),
                    // WP-S2.06 part 1
                    PaneKind::Navigator(k) => navigators
                        .entry(k)
                        .or_insert_with(|| NavigatorPane::of(k))
                        .show(ui, shell, &mut events),
                    PaneKind::Properties => properties.show(ui, shell, &mut events),
                    PaneKind::Find => find.show(ui, shell, &mut events),
                    // WP-S2.09 part 1
                    PaneKind::Jobs => crate::jobs::show(ui, shell, job_started, now, &mut events),
                    // WP-S2.09 part 3
                    PaneKind::Preview => crate::preview::show(ui, shell, &mut events),
                    PaneKind::History => crate::history::show(ui, shell, &mut events),
                    // WP-S2.08 part 6
                    PaneKind::Glossary => crate::glossary::show(ui, shell, &mut events),
                    PaneKind::PageView => {
                        // WP-S2.17 part 2: the HUD and the zoom control stand on the page view pane.
                        page_pane = Some(ui.max_rect());
                        canvas_area.show(ui, shell, canvas, &mut events);
                        *area_splitters = canvas_area.splitter_count(shell);
                        canvas_shown = true;
                    }
                });
                events.extend(headers);
                new_layout = dock.layout().filter(|l| l != shell.layout());
            } else {
                start_page::show(ui, &self.shell, &mut events);
            }
        });
        // WP-S2.17 part 2: the HUD, the zoom control and the tool options over the page view (doc 16a §5.5).
        self.page_pane = page_pane.filter(|_| canvas_shown);
        self.hud_rect = None;
        let mut options_card = None;
        if self.page_pane.is_none() {
            self.options_rect = None;
        }
        if let Some(pane) = self.page_pane {
            self.hud_rect = Some(crate::hud::show(&ctx, &self.shell, pane, &mut events));
            crate::hud::show_zoom(&ctx, &self.shell, pane, &mut events);
            options_card = self.tool_options(&ctx, pane, &mut events);
        }
        // OV2/OV6: the canvas painted before the HUD (whose place follows the pointer), so the chips keep off this
        // frame's HUD, zoom control and tool options card (W40b) from the next frame on; the next input repaints
        // anyway, no extra repaint.
        let chrome: Vec<egui::Rect> = match (self.page_pane, self.hud_rect) {
            (Some(pane), Some(hud)) => [hud, crate::hud::zoom_rect(&crate::theme::current(&ctx), pane)]
                .into_iter()
                // FT-1086: no obstacle for a ghost under the card.
                .chain(options_card.filter(|(_, faded)| !faded).map(|(rect, _)| rect))
                .collect(),
            _ => Vec::new(),
        };
        for group in 0..crate::canvas::GROUPS {
            self.canvas.set_chrome_obstacles(group, chrome.clone());
        }
        let palette_open = self.shell.palette().is_some();
        palette::show(
            &ctx,
            &self.shell,
            palette_open && !self.palette_was_open,
            self.command_bar.search_rect(),
            &mut events,
        );
        self.palette_was_open = palette_open;
        let picker_open = self.shell.picker().is_some();
        if !held_for_picker.is_empty() {
            ctx.input_mut(|i| i.events.extend(held_for_picker));
        }
        crate::picker::show(
            &ctx,
            &self.shell,
            picker_open && !self.picker_was_open,
            &mut events,
        );
        self.picker_was_open = picker_open;
        // WP-S2.18 D2: the Place tool's Join chooser (doc 16a §6.22).
        crate::chooser::show(
            &ctx,
            &self.shell,
            self.page_pane,
            &mut self.chooser_typing,
            &mut events,
        );
        crate::junction::show(&ctx, &self.shell, &mut events);
        crate::text_popup::show(&ctx, &self.shell, &mut self.text_buffer, &mut events);
        crate::parts_card::show(&ctx, &self.shell, &mut events);
        // FT-649: the parts import's column mapping step.
        crate::parts_map::show(&ctx, &self.shell, &mut events);
        // WP-S2.09 part 3: Show changes of the history panel.
        crate::history::show_changes(&ctx, &self.shell, &mut events);
        // WP-S2.09 part 1: the toasts and the notifications log.
        // WP-S2.17 part 4: at the canvas's top right (doc 16a §6.10).
        let bar_bottom = self.command_bar.bar_rect().map_or(0.0, |r| r.bottom());
        let mut anchor = crate::notifications::toast_anchor(&tokens, &ctx, self.page_pane, bar_bottom);
        // FT-822: the banner lies over the top edge of the canvas; the toasts start below it.
        if let Some(banner) = self.banner_overlay(&ctx, &tokens, bar_bottom, &mut events) {
            anchor.y = anchor.y.max(banner.bottom() + tokens.space_4);
        }
        let dialog_open = self.shell.dialog().is_some();
        // Doc 16a §6.12: under a modal the toasts go below its backdrop, which dims them with the rest.
        let under_modal = dialog_open || ctx.memory(|m| m.top_modal_layer().is_some());
        // FT-1027 (ID-1107; doc 16a §4.12, §6.10): each toast fades in and out over `motion.base` on the frame clock.
        let frame_time = ctx.input(|i| i.time);
        let fade = f64::from(tokens.motion_base) / 1000.0;
        // Hovering a toast pauses it (doc 16a §6.10): the clock of a toast the pointer was on in the last frame stands
        // still for the time since that frame.
        let step = self.toast_clock.map_or(0.0, |last| (frame_time - last).max(0.0));
        self.toast_clock = Some(frame_time);
        let was_paused = std::mem::take(&mut self.toast_hovered);
        for serial in &was_paused {
            if let Some(since) = self.toast_since.get_mut(serial) {
                *since += step;
            }
        }
        self.toast_opacity = self
            .shell
            .notifications()
            .visible()
            .iter()
            .map(|n| {
                let age = self.toast_since.get(&n.serial).map_or(0.0, |s| frame_time - s);
                (
                    n.serial,
                    crate::notifications::toast_opacity(age, TOAST_SECONDS, fade),
                )
            })
            .collect();
        let (hovered, focused) = crate::notifications::show_toasts(
            &ctx,
            &self.shell,
            anchor,
            under_modal,
            &self.toast_opacity,
            &mut events,
        );
        // FT-1088 (ID-1183; doc 16a §6.10): a hover pauses a toast only once the pointer has moved onto it (a
        // pointer-moved event in this frame) and keeps it paused while the pointer stays; a toast that appears under a
        // pointer at rest is not paused, so it fades on time. FT-1137 (ID-1225): a toast that holds the keyboard focus
        // is paused too.
        let pointer_moved = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::PointerMoved(_))));
        self.toast_hovered = hovered
            .into_iter()
            .filter(|serial| pointer_moved || was_paused.contains(serial))
            .chain(focused)
            .collect();
        crate::notifications::show_log(&ctx, &self.shell, &mut events);
        let opening = self.shell.dialog().map(|_| self.shell.dialog_generation());
        about::show(
            &ctx,
            &self.shell,
            opening.is_some() && opening != self.dialog_was,
            form_key,
            &mut events,
        );
        self.dialog_was = opening;
        // WP-S6.05 D11: the start-up offer, while no other dialog is open.
        if self.shell.dialog().is_none() {
            crate::startup::show(&ctx, &self.shell, &mut events);
        }
        // F1 resolves the context (a hovered menu item has taken the key already).
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1)) {
            events.push(Event::Action("help.documentation", Value::Null));
        }

        if canvas_shown {
            // The merged tile submission and the hidden-first eviction of every canvas (WP-S2.03 §6.8).
            self.canvas.finish_frame(&ctx, &self.shell);
        }
        // A native dialog that is open off the window's thread need not be modal (on Linux the portal dialog has no
        // owner window), and its answer carries the shell's earlier choices with it (`discard` after a Discard): what
        // the user does in the window in the meantime would be thrown away unasked. So while it is open the frame
        // paints and dispatches nothing of what the user did (keys, menus, panels, cards, canvas, dropped files).
        if self.pending_pick.is_some() {
            events.clear();
        }
        if !events.is_empty() {
            ctx.request_repaint();
        }
        // The toasts' timeouts are the window's (WP-S2.09 D6): 8 seconds each after the frame that first showed it.
        let visible: Vec<u64> = self
            .shell
            .notifications()
            .visible()
            .iter()
            .map(|n| n.serial)
            .collect();
        self.toast_since.retain(|s, _| visible.contains(s));
        for serial in visible {
            let age = frame_time - *self.toast_since.entry(serial).or_insert(frame_time);
            if age >= TOAST_SECONDS {
                events.push(Event::DismissToast(serial));
            } else if age < fade || age >= TOAST_SECONDS - fade {
                // In a fade: every frame until it ends.
                ctx.request_repaint();
            } else {
                let to_fade = Duration::from_secs_f64((TOAST_SECONDS - fade - age).max(0.0));
                ctx.request_repaint_after(to_fade.min(Duration::from_millis(500)));
            }
        }
        let dispatch_started = crate::clock::now();
        // The window repaints after an action that changed the model or the tool's marks without pointer motion
        // (an undo, a key-driven commit, a gesture's release; WP-S2.04 part 2, window check): once at once, and once
        // shortly after, when the page's new tiles have had time to arrive.
        let marks_before = (self.shell.revision(), self.shell.tool_overlay().len());
        for event in events {
            // FT-1130 (ID-1228 item 5): `ui.dismiss` while the app menu is open closes the menu and nothing else, as
            // Escape does.
            if matches!(event, Event::Action("ui.dismiss", _))
                && egui::Popup::is_id_open(&ctx, crate::command_bar::app_menu_id())
            {
                egui::Popup::close_all(&ctx);
                continue;
            }
            self.apply(event);
        }
        // WP-S2.05 part 3: what a copy wrote goes to the system clipboard (egui's `copy_text`).
        if let Some(text) = self.shell.take_clipboard_outbox() {
            ctx.copy_text(text);
        }
        if std::mem::take(&mut self.request_paste) {
            ctx.send_viewport_cmd(ViewportCommand::RequestPaste);
        }
        if (self.shell.revision(), self.shell.tool_overlay().len()) != marks_before {
            ctx.request_repaint();
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if canvas_shown {
            // The canvas's input phase includes dispatching the frame's actions (WP-S2.02 section 9).
            self.canvas
                .end_frame(crate::clock::now() - dispatch_started, &self.shell);
        } else if self.shell.has_project()
            && self.shell.first_run().is_none()
            && self.shell.wizard().is_none()
        {
            // The page pane is behind another tab of the dock: its tiles stay within a share of the cache (FT-356).
            self.canvas.hide();
        } else {
            // No canvas on screen (start page, wizard, first run): its tiles and queued jobs go.
            self.canvas.release();
        }
        self.open_documentation();
        self.open_results();
        self.open_log_folder();
        self.print_requests(&ctx);

        // The layout the user made is the shell's; it is written a second after the last change.
        let now = ctx.input(|i| i.time);
        if let Some(layout) = new_layout {
            self.shell.set_layout(layout, now);
        }
        self.shell.persist_layout_if_due(now);
        if self.shell.layout_pending() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        let title = self.shell.title();
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        if self.shell.should_quit() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// The shell's shortcuts. Routing order: a focused text field, then an open popup or dialog, then the
    /// shortcuts; Escape closes the topmost popup or dialog and otherwise goes to the active canvas tool through
    /// `ui.dismiss` (WP-S2.03 §7.2). F1 is handled at the end of the frame, in every state. While a canvas has the
    /// focus, `[` and `]` are matched as the physical keys (no AltGr on QWERTZ and AZERTY, doc 16 §4.4).
    fn route_keys(&mut self, ctx: &egui::Context, events: &mut Vec<Event>) {
        let palette_open = self.shell.palette().is_some();
        let dialog_open = self.shell.dialog().is_some();
        // The junction sequence editor is a popup like the picker (WP-S2.04 part 2): Escape closes it, its own keys
        // stay with it. The text popup is one too (WP-S2.04 part 4b §10.5).
        let picker_open = self.shell.picker().is_some()
            // WP-S2.18 D2: the Join chooser is a popup too.
            || self.shell.chooser().is_some()
            || self.shell.junction_editor().is_some()
            || self.shell.text_popup().is_some()
            // WP-S2.09 §6.5: the notifications log is a popup too.
            || self.shell.notifications().popup_open();
        let overlay = palette_open || dialog_open || picker_open;
        // WP-S2.17 part 2: Escape closes the app menu (with its submenus), then the collapsed bar's command-row popover.
        if egui::Popup::is_id_open(ctx, crate::command_bar::app_menu_id())
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            egui::Popup::close_all(ctx);
            return;
        }
        if self.command_bar.popover_open()
            && !overlay
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            self.command_bar.close_popover();
            return;
        }
        if overlay && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return;
        }
        // WP-S6.05 D11: the start-up offer is modal; Escape is its Not now.
        if self.shell.startup_offer().is_some() {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
                events.push(Event::Action("startup.dismiss", Value::Null));
            }
            return;
        }
        if dialog_open || picker_open || (ctx.text_edit_focused() && !palette_open) {
            return;
        }
        // W46.4 i3: Escape with a Properties editor open is the editor's (`props.cancel`), whether or not its field
        // still holds the focus: egui drops the focus on Escape before the frame, so `ui.dismiss` took it and the field
        // then committed on the lost focus (refused: nothing was being edited).
        if self.properties.field_open() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            return;
        }
        if !palette_open
            && self.shell.has_project()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return;
        }
        // FT-519: the history panel's row keys while it has the focus.
        if !palette_open && crate::history::keys(ctx, events) {
            return;
        }
        // WP-S2.17 part 3: Ctrl+1 to Ctrl+9 pick the left rail's entries (doc 16a §5.4).
        if !palette_open
            && self.shell.has_project()
            && self.shell.wizard().is_none()
            && crate::rail::keys(ctx, &self.shell, events)
        {
            return;
        }
        // FT-410: Ctrl+Enter applies an open parts import card from anywhere (the card is a window of its own).
        if !palette_open
            && self.shell.import_card_open()
            && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter))
        {
            events.push(Event::Action("preview.apply", Value::Null));
            return;
        }
        // The focused context's chords first, then the global ones (WP-S2.03 §6.6); while the navigator has the
        // focus its own keys (arrows, Home, End, Enter) stay with it.
        let focused = match self.shell.focus() {
            Some(Focus::Navigator(_)) => Context::Navigator,
            Some(Focus::Canvas(_)) => Context::Canvas,
            Some(Focus::Properties) => Context::Properties,
            Some(Focus::Find) => Context::Find,
            Some(Focus::CommandBar | Focus::History) | None => Context::Global,
        };
        // WP-S2.17 part 2 (doc 16a §5.9): on the command bar Left and Right move between the tabs.
        if self.shell.focus() == Some(Focus::CommandBar) && !palette_open {
            let step = ctx.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    -1
                } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    1
                } else {
                    0
                }
            });
            if step != 0 {
                let tab = crate::command_bar::step_tab(self.shell.command_tab(), step);
                events.push(Event::Action("view.command_tab", json!({ "tab": tab.word() })));
                ctx.memory_mut(|m| m.request_focus(crate::command_bar::tab_id(tab)));
                return;
            }
        }
        // The chords bound by physical key (`]` and `[` in the canvas, any `phys:` key of a keymap, WP-S2.08 D6) are
        // matched on the key's position, so they need no AltGr on QWERTZ and AZERTY: the focused context's first, the
        // most modifiers first; a chord without Shift also takes the key with Shift.
        if !palette_open {
            let mut physical: Vec<&Binding> = self
                .bindings
                .iter()
                .filter(|b| b.physical && (b.context == Context::Global || b.context == focused))
                .collect();
            physical.sort_by_key(|b| {
                let m = b.shortcut.modifiers;
                (
                    std::cmp::Reverse(usize::from(m.shift) + usize::from(m.alt) + usize::from(m.command)),
                    b.context != focused,
                )
            });
            let hit = ctx.input_mut(|i| {
                let mut hit = None;
                i.events.retain(|e| match e {
                    // A platform that reports no physical key (a test's) has its logical key matched instead.
                    egui::Event::Key {
                        key,
                        physical_key,
                        pressed: true,
                        modifiers,
                        ..
                    } if hit.is_none() => {
                        let k = physical_key.unwrap_or(*key);
                        let found = physical.iter().find(|b| {
                            let want = b.shortcut.modifiers;
                            b.shortcut.logical_key == k
                                && want.command == (modifiers.command || modifiers.ctrl)
                                && want.alt == modifiers.alt
                                && (!want.shift || modifiers.shift)
                        });
                        match found {
                            Some(b) => {
                                hit = Some(b.action);
                                false
                            }
                            None => true,
                        }
                    }
                    _ => true,
                });
                hit
            });
            if let Some(action) = hit {
                let (action, params) = self.shell.shortcut_action(action);
                self.shell.note_shortcut_use(action);
                events.push(Event::Action(action, params));
                return;
            }
        }
        // WP-S2.05 part 3: the platform layer turns Ctrl+C, Ctrl+X and Ctrl+V into clipboard events, not key presses
        // (egui-winit); on the canvas they run the clipboard actions, a paste with the system clipboard's text. A text
        // field never gets here (the return above), so the grid and the find pane keep egui's own copy and paste.
        if focused == Context::Canvas && !palette_open {
            let clip = ctx.input_mut(|i| {
                let mut hit = None;
                i.events.retain(|e| match e {
                    egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) if hit.is_none() => {
                        hit = Some(e.clone());
                        false
                    }
                    _ => true,
                });
                hit
            });
            match clip {
                Some(egui::Event::Copy) => {
                    events.push(Event::Action("edit.copy", Value::Null));
                    return;
                }
                Some(egui::Event::Cut) => {
                    events.push(Event::Action("edit.cut", Value::Null));
                    return;
                }
                Some(egui::Event::Paste(text)) => {
                    events.push(Event::Action("edit.paste_clip", json!({ "text": text })));
                    return;
                }
                _ => {}
            }
        }
        // WP-S2.06 part 3: in a navigator's table the clipboard events run the table's copy and paste (§8.6).
        if focused == Context::Navigator
            && !palette_open
            && let Some(kind) = self.shell.focused_navigator()
            && self.shell.nav(kind).is_some_and(|n| n.table_mode().is_some())
        {
            let clip = ctx.input_mut(|i| {
                let mut hit = None;
                i.events.retain(|e| match e {
                    egui::Event::Copy | egui::Event::Paste(_) if hit.is_none() => {
                        hit = Some(e.clone());
                        false
                    }
                    _ => true,
                });
                hit
            });
            match clip {
                Some(egui::Event::Copy) => {
                    events.push(Event::Action("table.copy", json!({ "kind": kind.word() })));
                    return;
                }
                Some(egui::Event::Paste(text)) => {
                    events.push(Event::Action(
                        "table.paste",
                        json!({ "kind": kind.word(), "text": text }),
                    ));
                    return;
                }
                _ => {}
            }
        }
        // One list over the focused context and the global one, most modifiers first (egui matches an extra Shift
        // or Alt logically, so Alt+Left must be asked before Left), the focused context first on a tie.
        let mut candidates: Vec<&Binding> = self
            .bindings
            .iter()
            .filter(|b| !b.physical && (b.context == Context::Global || b.context == focused))
            .collect();
        candidates.sort_by_key(|b| {
            let m = b.shortcut.modifiers;
            (
                std::cmp::Reverse(usize::from(m.shift) + usize::from(m.alt) + usize::from(m.command)),
                b.context != focused,
            )
        });
        for binding in candidates {
            if matches!(binding.action, "ui.dismiss" | "help.documentation")
                || (palette_open && binding.action != "tools.palette")
                || (focused == Context::Navigator && keys::is_navigator_key(&binding.shortcut))
                || (focused == Context::Properties
                    && binding.context != Context::Properties
                    && crate::properties::is_grid_key(&binding.shortcut))
                // WP-S2.05 part 2: the find pane's own keys (the arrows, Home, End, Enter) stay with it.
                || (focused == Context::Find
                    && binding.context != Context::Find
                    && crate::properties::is_grid_key(&binding.shortcut))
            {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&binding.shortcut)) {
                // WP-S2.05 part 2: Ctrl+Enter in the find pane applies its open replace card.
                let replace_card =
                    self.shell.properties().card().is_some_and(|c| {
                        matches!(c.origin, ew_viewmodel::properties::CardOrigin::Find { .. })
                    });
                let (action, params) = if binding.action == "find.replace_all" && replace_card {
                    ("preview.apply", Value::Null)
                } else {
                    self.shell.shortcut_action(binding.action)
                };
                // WP-S2.08 D3 (PL-4): a shortcut invocation counts for the palette's recents.
                self.shell.note_shortcut_use(action);
                events.push(Event::Action(action, params));
                return;
            }
        }
    }

    /// Alt pressed and released with nothing else, or F10, opens the app menu (doc 16a §5.2); not while a dialog or a
    /// file dialog is open.
    fn app_menu_keys(&mut self, ctx: &egui::Context) {
        let (alt, other, f10) = ctx.input_mut(|i| {
            let other = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::PointerButton { pressed: true, .. }
                        | egui::Event::Text(_)
                )
            });
            // F10 alone: `consume_key` matches logically and would take Shift+F10, the context menu key (FR-UI-031).
            let f10 = i.modifiers.is_none() && i.consume_key(Modifiers::NONE, Key::F10);
            (i.modifiers.alt, other, f10)
        });
        let mut open = f10;
        if alt && !self.alt_down {
            self.alt_alone = !other;
        } else if alt && other {
            self.alt_alone = false;
        }
        if !alt && self.alt_down && self.alt_alone {
            open = true;
        }
        if !alt {
            self.alt_alone = false;
        }
        self.alt_down = alt;
        if open && self.shell.dialog().is_none() && self.pending_pick.is_none() {
            egui::Popup::open_id(ctx, crate::command_bar::app_menu_id());
        }
    }

    /// The tool options card at the page view's top left (WP-S2.17 part 2): the fields of the IP tool (WP-S2.04 §9.1),
    /// the Text tool's binding, the Wire marker's fields and the Structure box's style and fields (WP-S2.04 §10.5),
    /// which the removed tool strip held, and the layout place options (WP-S4.02 part 3). Shown only while one of those
    /// tools runs (or Select drags a placement); returns the card's frame (the canvas chips keep off it, W40b).
    fn tool_options(
        &mut self,
        ctx: &egui::Context,
        pane: egui::Rect,
        events: &mut Vec<Event>,
    ) -> Option<(egui::Rect, bool)> {
        let tool = self.shell.tool().name();
        let (ip, text, marker, structure_box) = (
            tool == "ip",
            tool == "text",
            tool == "marker",
            tool == "structure_box",
        );
        // WP-S4.02 part 3: the layout place tool, or a placement dragged by Select.
        let layout = self
            .shell
            .layout_options_shown()
            .then(|| self.shell.layout_card_options());
        // WP-S4.02 part 4 (D11): Draw rail's and Draw duct's part, grid step and duct size.
        let draw = self
            .shell
            .draw_card()
            .map(|card| (card, self.shell.layout_options().grid_mm));
        if !(ip || text || marker || structure_box || layout.is_some() || draw.is_some()) {
            self.options_rect = None;
            return None;
        }
        let t = crate::theme::current(ctx);
        let externals = if ip || structure_box {
            self.shell.external_choices()
        } else {
            Vec::new()
        };
        let text_bind = if text { self.shell.text_bind() } else { None };
        // FT-851: the IP name field shows the typed name, else the ghost's proposal as its hint.
        let ip_typed = self.shell.ip_typed_name().map(str::to_owned);
        let ip_proposal = match self.shell.tool_ghost() {
            Some(ew_viewmodel::tools::Ghost::Ip { name, .. }) if ip && !name.is_empty() => Some(name),
            _ => None,
        };
        // FT-1086 (ID-1183): see below.
        let faded = self
            .options_rect
            .zip(self.ghost_screen_point())
            .is_some_and(|(rect, ghost)| rect.contains(ghost));
        let at = egui::pos2(
            pane.left() + t.space_4,
            pane.top() + t.size_dock_tab_bar + t.space_4,
        );
        let App {
            ip_name,
            marker_csa,
            marker_colours,
            structure_caption,
            structure_location,
            ..
        } = self;
        // FT-1086 (ID-1183): only the controls take the pointer. The card is two layers: its frame and title, which
        // let the pointer through to the canvas beneath (a click on them reaches the sheet), and, over it, a band
        // holding the controls, which are the card's only interactive part. While the tool's ghost is under the card
        // (by the rect the last frame drew, so the card never flips) it is drawn at 40 % and is no chip obstacle.
        let opacity = if faded { OPTIONS_FADED_OPACITY } else { 1.0 };
        let title = strings::text(key!("hud-tool-options"));
        let title_font = crate::theme::font(&t.type_body_strong);
        let title_size = ctx.fonts_mut(|f| {
            f.layout_no_wrap(title.to_owned(), title_font.clone(), crate::theme::NONE)
                .size()
        });
        let frame = crate::components::card_frame(&t).shadow(crate::theme::shadow(&t.elevation_1));
        let margin = frame.total_margin();
        let gap = ctx.global_style().spacing.item_spacing.y;
        let band_at = at + egui::vec2(margin.left, margin.top + title_size.y + gap);
        // ID-1139: the areas are named per tool, so a tool's card sizes itself on a sizing pass rather than inheriting
        // the last tool's size as its bound.
        let id = egui::Id::unique(("tool-options", tool));
        // FT-1115 (ID-1225): the band takes the pointer only on its controls. A container's hit rectangle is its clip
        // rectangle cut to its extent, so the band's area (constrained to nothing, and not moved by it) and its row
        // leave a clip of nothing behind them as they end: a click in a gap between the controls reaches the sheet.
        let screen = ctx.content_rect();
        let band = egui::Area::new(id.with("controls"))
            .fixed_pos(band_at)
            .order(egui::Order::Middle)
            .constrain_to(egui::Rect::NOTHING)
            .constrain(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                ui.set_opacity(opacity);
                ui.horizontal(|ui| {
                    // WP-S2.04 §9.1: the IP tool's name field and external-reference field.
                    if ip {
                        crate::tool_view::ip_fields(
                            ui,
                            ip_name,
                            ip_typed.as_deref(),
                            ip_proposal.as_deref(),
                            &externals,
                            events,
                        );
                    }
                    // WP-S2.04 §10.5: the Text tool's binding.
                    if text {
                        crate::tool_view::text_fields(ui, text_bind.as_deref(), events);
                    }
                    // WP-S2.04 §10.5: the Wire marker's fields and the Structure box's style and fields.
                    if marker {
                        crate::tool_view::marker_fields(ui, marker_csa, marker_colours, events);
                    }
                    if structure_box {
                        crate::tool_view::structure_box_fields(
                            ui,
                            &externals,
                            structure_caption,
                            structure_location,
                            events,
                        );
                    }
                    if let Some(options) = layout {
                        crate::tool_view::layout_fields(ui, options, events);
                    }
                    if let Some((card, grid_mm)) = &draw {
                        crate::tool_view::draw_fields(ui, card, *grid_mm, events);
                    }
                    ui.set_clip_rect(egui::Rect::NOTHING);
                });
                ui.set_clip_rect(egui::Rect::NOTHING);
            });
        let band_size = band.response.rect.size();
        let card = egui::Area::new(id)
            .interactable(false)
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.set_opacity(opacity);
                // WP-S2.17 part 4: a card of doc 16a §6.9 (panel fill, a 1-px border, `radius.md`, `space.4` padding,
                // the title in `type.body.strong`) with `elevation.1`, as it floats over the canvas.
                frame.show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(title)
                            .font(title_font)
                            .color(t.text_primary.color32()),
                    );
                    ui.allocate_space(band_size);
                });
            });
        // The controls stand over the frame, which was shown after them.
        ctx.move_to_top(band.response.layer_id);
        self.options_rect = Some(card.response.rect);
        Some((card.response.rect, faded))
    }

    /// The screen point of the tool's ghost on the focused canvas as its last frame drew it: its insertion point, else
    /// the pointer's (FT-1086); `None` without a ghost.
    fn ghost_screen_point(&self) -> Option<egui::Pos2> {
        use ew_viewmodel::tools::Ghost;
        let ghost = self.shell.tool_ghost()?;
        let canvas = self.shell.canvas()?;
        let at = match ghost {
            Ghost::Place { at_nm, .. }
            | Ghost::Paste { at_nm, .. }
            | Ghost::Ip { at_nm, .. }
            | Ghost::Text { at_nm, .. } => at_nm.map(|p| canvas.page().snapper.page_to_sheet(p)),
            _ => None,
        }
        .or_else(|| canvas.pointer())?;
        let group = self.shell.area().map_or(0, |a| a.focused);
        self.canvas.drawn_screen_of(group, at)
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Action(action, params) => self.dispatch(action, &params),
            Event::PaletteRun => self.run(None, ShellModel::run_palette_selection),
            Event::PaletteClick(i) => {
                self.shell.select_palette_entry(i);
                self.run(None, ShellModel::run_palette_selection);
            }
            Event::PaletteQuery(query) => self.shell.set_palette_query(&query),
            Event::PaletteMove(delta) => self.shell.move_palette_selection(delta),
            Event::PaletteField(i, text) => self.shell.set_palette_field(i, &text),
            Event::Confirm(answer) => self.run(None, |shell| shell.answer_confirm(answer)),
            Event::GotoText(text) => self.shell.set_goto_text(&text),
            Event::GotoGo(text) => self.dispatch("nav.goto", &json!({ "text": text })),
            Event::WorkspaceText(text) => self.shell.set_workspace_text(&text),
            Event::FormField(name, text) => self.shell.set_form_field(name, &text),
            Event::FormTableCell(row, column, text) => self.shell.set_form_table_cell(row, column, &text),
            Event::FormTableAdd => self.shell.add_form_table_row(),
            Event::FormTableRemove(row) => self.shell.remove_form_table_row(row),
            // A refusal stays in the dialog, so it is not made a banner as well.
            Event::FormSubmit => {
                self.shell.submit_form();
            }
            Event::DismissBanner => self.shell.dismiss_banner(),
            Event::DismissToast(serial) => self.shell.dismiss_toast(serial),
            Event::DisabledClick(text) => self.shell.explain_disabled(text),
            Event::JunctionRow(row) => self.shell.set_junction_row(row),
            // WP-S6.05 D16: the bundled copy when there is one, else the repository's.
            Event::OpenPrivacy => {
                let target = ew_viewmodel::about::privacy_target(&self.shell);
                if !self.platform.open_url(&target) {
                    // The file's path as Windows users read it: without the verbatim prefix `canonicalize` gives.
                    let at = self
                        .shell
                        .config()
                        .resources
                        .privacy_file
                        .as_ref()
                        .map_or(target, |p| {
                            p.to_string_lossy().trim_start_matches(r"\\?\").to_owned()
                        });
                    let text = strings::format(key!("privacy-open-failed"), &[&at]);
                    self.shell.show_banner(&text);
                }
            }
        }
    }

    /// The file dialogs the shell asked for: their answers are dispatched like any other input. The platform's native
    /// dialogs run off the window's thread (FT-467), so a dialog's answer is read here, at the start of a frame, once
    /// its thread has sent it; while it is open the shell's next prompt waits. A platform that answers at once (the
    /// tests') is answered in the same frame.
    fn answer_prompts(&mut self, ctx: &egui::Context) {
        loop {
            if let Some((request, rx)) = self.pending_pick.take() {
                match rx.try_recv() {
                    Ok(pick) => self.answer_prompt(ctx, request, pick),
                    Err(TryRecvError::Empty) => {
                        self.pending_pick = Some((request, rx));
                        return;
                    }
                    // The dialog's thread ended without an answer (it panicked): as if the user cancelled.
                    Err(TryRecvError::Disconnected) => self.answer_prompt(ctx, request, Pick::Cancelled),
                }
                continue;
            }
            let Some(request) = self.shell.take_prompt() else {
                return;
            };
            match self.platform.start_pick(request.prompt, ctx) {
                PickReply::Now(pick) => self.answer_prompt(ctx, request, pick),
                PickReply::Later(rx) => self.pending_pick = Some((request, rx)),
            }
        }
    }

    /// Dispatches the answer `pick` to the dialog of `request`.
    fn answer_prompt(&mut self, ctx: &egui::Context, request: PromptRequest, pick: Pick) {
        let path = match pick {
            Pick::Path(path) => path,
            Pick::Cancelled => {
                self.prompt_answered(ctx);
                return;
            }
            Pick::Unavailable => {
                self.shell
                    .show_banner(strings::text(key!("banner-no-file-dialog")));
                self.prompt_answered(ctx);
                return;
            }
        };
        let mut params = request.params.as_object().cloned().unwrap_or_default();
        params.insert(
            request.param.to_owned(),
            Value::String(path.to_string_lossy().into_owned()),
        );
        // WP-S2.09 part 1: the PDF export is a job now (its request carries `replace`: the save dialog asked).
        self.dispatch(request.action, &Value::Object(params));
        self.prompt_answered(ctx);
    }

    /// The menu state after a dialog: close every popup and request the frame that paints the result.
    fn prompt_answered(&mut self, ctx: &egui::Context) {
        // The menu the dialog was opened from is closed in the frame that paints the answer.
        egui::Popup::close_all(ctx);
        ctx.request_repaint();
        // A native dialog may still be closing while that frame paints (the window not yet enabled again); a second
        // frame shortly after shows the answer's state (the tool's hint) without waiting for a pointer move (R30 F3).
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    /// Open PDF (WP-S2.09 §6.4): the written file in the system's viewer; a banner when it could not be opened.
    fn open_results(&mut self) {
        while let Some(url) = self.shell.take_open_request() {
            if !self.platform.open_url(&url) {
                self.shell.show_banner(strings::text(key!("open-failed")));
            }
        }
    }

    /// The start-up offer's Open log folder (WP-S6.05 D11): the folder in the system's file manager; the offer says
    /// where it is when it could not be opened (S7).
    fn open_log_folder(&mut self) {
        if let Some((url, folder)) = self.shell.take_log_folder_request()
            && !self.platform.open_url(&url)
        {
            self.shell.log_folder_failed(&folder);
        }
    }

    /// The print pre-dialog's Print… and Open in PDF viewer (WP-S3.07 part 3, ruling 3): the ends that are in, each
    /// with its one toast, then the new requests handed to the platform.
    fn print_requests(&mut self, ctx: &egui::Context) {
        let mut ended = Vec::new();
        self.pending_prints.retain(|rx| match rx.try_recv() {
            Ok(end) => {
                ended.push(end);
                false
            }
            Err(TryRecvError::Empty) => true,
            // The print's thread ended without an answer (it panicked).
            Err(TryRecvError::Disconnected) => {
                ended.push(PrintEnd::Failed(
                    strings::text(key!("job-cause-internal")).to_owned(),
                ));
                false
            }
        });
        for end in ended {
            self.shell.print_done(end);
        }
        while let Some(request) = self.shell.take_print_request() {
            match self.platform.print(request, ctx) {
                PrintReply::Now(end) => self.shell.print_done(end),
                PrintReply::Later(rx) => self.pending_prints.push(rx),
            }
        }
    }

    /// The documentation page F1 asked for: opened in the system browser when the user guide is installed, else a
    /// banner says it is not.
    fn open_documentation(&mut self) {
        while let Some(request) = self.shell.take_doc_request() {
            let opened = request
                .url
                .as_deref()
                .is_some_and(|url| self.platform.open_url(url));
            if !opened {
                self.shell.show_banner(strings::text(key!("doc-missing")));
            }
        }
    }
}

/// Whether `e` is keyboard input (a key, typed text, an IME step, a clipboard key): what an open picker owns.
fn is_keyboard_event(e: &egui::Event) -> bool {
    matches!(
        e,
        egui::Event::Key { .. }
            | egui::Event::Text(_)
            | egui::Event::Ime(_)
            | egui::Event::Paste(_)
            | egui::Event::Copy
            | egui::Event::Cut
    )
}

/// The first of the dropped files that doc 16 §4.8 maps onto the Image tool: every file that is no QElectroTech
/// project, parts table, spreadsheet or project folder or archive (those drops are other flows). A file the asset store
/// does not accept is refused by `image.file` with the store's text.
pub(crate) fn dropped_image(files: &[PathBuf]) -> Option<&PathBuf> {
    files.iter().find(|f| {
        let ext = f
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        !f.is_dir() && !matches!(ext.as_str(), "qet" | "ew" | "ewz" | "csv" | "xlsx")
    })
}

#[cfg(test)]
mod drop_tests {
    use super::*;

    #[test]
    fn only_files_of_no_other_drop_flow_go_to_the_image_tool() {
        let p = |s: &str| PathBuf::from(s);
        let files = [p("plant.QET"), p("parts.csv"), p("logo.svg"), p("photo.png")];
        assert_eq!(dropped_image(&files), Some(&files[2]));
        assert_eq!(dropped_image(&files[..2]), None);
        assert_eq!(dropped_image(&[p("noext")]), Some(&p("noext")));
        assert_eq!(dropped_image(&[]), None);
    }
}
