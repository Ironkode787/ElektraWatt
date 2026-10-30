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
use ew_viewmodel::layout::PaneKind;
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
    /// The banner's own button.
    DismissBanner,
    /// The privacy button of the About dialog and of the first-run privacy step: `PRIVACY.md` in the system browser.
    OpenPrivacy,
    /// The junction sequence editor's focused row (WP-S2.04 part 2; a view change of the popup).
    JunctionRow(usize),
    /// A toast's Close (WP-S2.09 §6.5): the toast leaves the screen, the log keeps it.
    DismissToast(u64),
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
    /// The find pane (WP-S2.05 part 2).
    find: FindPane,
    /// The visible toasts by serial and the frame time (egui's `input.time`, seconds) of the frame that first painted
    /// each: a toast goes after 8 seconds measured by the window, never by the model (WP-S2.09 D6). The frame clock is
    /// the wall clock in the window and the harness's steps in a test, so no test depends on how fast a loaded machine
    /// runs (FT-524).
    toast_since: std::collections::BTreeMap<u64, f64>,
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
    title: String,
    palette_was_open: bool,
    /// Whether the picker was open in the previous frame (its filter field takes the focus when it opens).
    picker_was_open: bool,
    /// The Join chooser's type-ahead text (WP-S2.18 D2).
    chooser_typing: crate::chooser::TypeAhead,
    dialog_was_open: bool,
    /// The file dialog that is open off the window's thread, with the request it answers (FT-467).
    pending_pick: Option<(PromptRequest, Receiver<Pick>)>,
    /// The `ui_scale` last given to egui: the zoom is set when the setting changes, not every frame, so that
    /// egui's own zoom keys keep working in between.
    applied_scale: Option<f32>,
    /// The zoom factor of the previous frame, for the window memory (its report of a frame uses that zoom).
    window_zoom: Option<f32>,
    /// The theme and density settings last given to egui.
    applied_theme: Option<(String, String)>,
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
    /// Whether the window had the operating system's focus in the last frame (FT-514).
    window_focused: Option<bool>,
    /// How many times the window regained the focus and asked for the frames that repaint it (FT-514; tests).
    focus_repaints: u64,
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
            find: FindPane::new(),
            toast_since: std::collections::BTreeMap::new(),
            job_started: std::collections::BTreeMap::new(),
            waker_set: false,
            area_splitters: 0,
            preview_view: PageView::new(),
            dock,
            initial,
            bindings: keys::bindings(),
            title: String::new(),
            palette_was_open: false,
            picker_was_open: false,
            chooser_typing: crate::chooser::TypeAhead::default(),
            dialog_was_open: false,
            pending_pick: None,
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
            window_focused: None,
            focus_repaints: 0,
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
        let outcome = self.shell.dispatch(action, params);
        self.report(outcome);
    }

    /// A failure or incomplete Cut refusal becomes a banner when the shell has not shown one of its own. A refusal the
    /// navigator's inline field shows under itself (an F2 tag's, FT-633) is not made a banner as well (cycle 37, window
    /// check W37: one fact in one place, doc 16a P3).
    fn report(&mut self, outcome: Outcome) {
        if self.shell.nav_rename_error().is_some() {
            return;
        }
        let message = match outcome {
            Outcome::Failed { message } => message,
            Outcome::Refused {
                reason_key: "reason-cut-incomplete-items",
            } => strings::text("reason-cut-incomplete-items").to_owned(),
            _ => return,
        };
        if self.shell.banner().is_none() {
            self.shell.show_banner(&message);
        }
    }

    /// Paints one frame into the root `ui` of the window.
    pub fn ui(&mut self, ui: &mut Ui) {
        let _frame = tracing::trace_span!("frame").entered();
        let ctx = ui.ctx().clone();
        // `ui_scale` is a factor on the native scale: egui's zoom factor is exactly that, and follows the window
        // to another monitor. It is set once when the setting changes.
        let scale = self.shell.settings().ui_scale;
        if self.applied_scale != Some(scale) {
            ctx.set_zoom_factor(scale);
            self.applied_scale = Some(scale);
        }
        // WP-S2.17 part 1: the theme and the density as tokens (doc 16a §4.14), applied when either setting changes;
        // `system` lets egui follow the operating system between Light and Dark (D5).
        let appearance = (
            self.shell.settings().theme.clone(),
            self.shell.settings().density.clone(),
        );
        if self.applied_theme.as_ref() != Some(&appearance) {
            crate::theme::apply_setting(&appearance.0, &appearance.1, &ctx);
            // A visual run draws the caret always (WP-S6.13 part 4 §3.1), so a frame never catches it off.
            if crate::clock::on_frame_clock() {
                ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
            }
            self.applied_theme = Some(appearance);
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
        self.route_keys(&ctx, &mut events);
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
        if let Some(banner) = self.shell.banner() {
            // WP-S2.17 part 4: the banner of doc 16a §6.11 (the warning subtle fill, its icon, one sentence, one
            // Primary action, a Subtle dismiss).
            let frame = egui::Frame::NONE
                .fill(tokens.warning_subtle.color32())
                .inner_margin(egui::Margin::symmetric(
                    tokens.space_4 as i8,
                    tokens.space_2 as i8,
                ));
            Panel::top("banner").frame(frame).show(ui, |ui| {
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
                            strings::text("action-file-discard-journal"),
                            true,
                            0.0,
                        )
                        .clicked()
                    {
                        events.push(Event::Action("file.discard_journal", Value::Null));
                    }
                    if crate::components::button(
                        ui,
                        crate::components::Kind::Subtle,
                        strings::text("banner-dismiss"),
                        true,
                        0.0,
                    )
                    .clicked()
                    {
                        events.push(Event::DismissBanner);
                    }
                });
            });
        }
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
                // Doc 16a §5.7: the Messages tab's count chip shows the open findings.
                let counts = [(
                    PaneKind::Navigator(ew_viewmodel::navigators::NavKind::Messages),
                    shell
                        .messages_counts()
                        .map_or(0, |c| c.errors + c.warnings + c.infos),
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
        if let Some(pane) = self.page_pane {
            self.hud_rect = Some(crate::hud::show(&ctx, &self.shell, pane, &mut events));
            crate::hud::show_zoom(&ctx, &self.shell, pane, &mut events);
            self.tool_options(&ctx, pane, &mut events);
        }
        // OV2/OV6: the canvas painted before the HUD (whose place follows the pointer), so the chips keep off this
        // frame's HUD and zoom control from the next frame on; the next input repaints anyway, no extra repaint.
        let chrome: Vec<egui::Rect> = match (self.page_pane, self.hud_rect) {
            (Some(pane), Some(hud)) => vec![hud, crate::hud::zoom_rect(&crate::theme::current(&ctx), pane)],
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
        let anchor = crate::notifications::toast_anchor(&tokens, &ctx, self.page_pane, bar_bottom);
        let dialog_open = self.shell.dialog().is_some();
        // Doc 16a §6.12: under a modal the toasts go below its backdrop, which dims them with the rest.
        let under_modal = dialog_open || ctx.memory(|m| m.top_modal_layer().is_some());
        crate::notifications::show_toasts(&ctx, &self.shell, anchor, under_modal, &mut events);
        crate::notifications::show_log(&ctx, &self.shell, &mut events);
        about::show(
            &ctx,
            &self.shell,
            dialog_open && !self.dialog_was_open,
            &mut events,
        );
        self.dialog_was_open = dialog_open;
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
        let frame_time = ctx.input(|i| i.time);
        for serial in visible {
            let since = *self.toast_since.entry(serial).or_insert(frame_time);
            if frame_time - since >= TOAST_SECONDS {
                events.push(Event::DismissToast(serial));
            } else {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
        let dispatch_started = crate::clock::now();
        // The window repaints after an action that changed the model or the tool's marks without pointer motion
        // (an undo, a key-driven commit, a gesture's release; WP-S2.04 part 2, window check): once at once, and once
        // shortly after, when the page's new tiles have had time to arrive.
        let marks_before = (self.shell.revision(), self.shell.tool_overlay().len());
        for event in events {
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
        if dialog_open || picker_open || (ctx.text_edit_focused() && !palette_open) {
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
        if focused == Context::Canvas && !palette_open {
            let bracket = ctx.input_mut(|i| {
                let mut hit = None;
                i.events.retain(|e| match e {
                    egui::Event::Key {
                        physical_key: Some(k @ (Key::OpenBracket | Key::CloseBracket)),
                        pressed: true,
                        modifiers,
                        ..
                    } if hit.is_none() && !modifiers.command && !modifiers.ctrl && !modifiers.alt => {
                        hit = Some(*k);
                        false
                    }
                    _ => true,
                });
                hit
            });
            if let Some(k) = bracket {
                let action = if k == Key::CloseBracket {
                    "cp.next"
                } else {
                    "cp.previous"
                };
                events.push(Event::Action(action, Value::Null));
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
            .filter(|b| b.context == Context::Global || b.context == focused)
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
    /// which the removed tool strip held. Shown only while one of those tools runs.
    fn tool_options(&mut self, ctx: &egui::Context, pane: egui::Rect, events: &mut Vec<Event>) {
        let tool = self.shell.tool().name();
        let (ip, text, marker, structure_box) = (
            tool == "ip",
            tool == "text",
            tool == "marker",
            tool == "structure_box",
        );
        if !(ip || text || marker || structure_box) {
            return;
        }
        let t = crate::theme::current(ctx);
        let externals = if ip || structure_box {
            self.shell.external_choices()
        } else {
            Vec::new()
        };
        let text_bind = if text { self.shell.text_bind() } else { None };
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
        egui::Area::new(egui::Id::unique("tool-options"))
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                // WP-S2.17 part 4: a card of doc 16a §6.9 (panel fill, a 1-px border, `radius.md`, `space.4` padding,
                // the title in `type.body.strong`) with `elevation.1`, as it floats over the canvas.
                crate::components::card_frame(&t)
                    .shadow(crate::theme::shadow(&t.elevation_1))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(strings::text("hud-tool-options"))
                                .font(crate::theme::font(&t.type_body_strong))
                                .color(t.text_primary.color32()),
                        );
                        ui.horizontal(|ui| {
                            // WP-S2.04 §9.1: the IP tool's name field and external-reference field.
                            if ip {
                                crate::tool_view::ip_fields(ui, ip_name, &externals, events);
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
                        });
                    });
            });
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Action(action, params) => self.dispatch(action, &params),
            Event::PaletteRun => {
                let outcome = self.shell.run_palette_selection();
                self.report(outcome);
            }
            Event::PaletteClick(i) => {
                self.shell.select_palette_entry(i);
                let outcome = self.shell.run_palette_selection();
                self.report(outcome);
            }
            Event::PaletteQuery(query) => self.shell.set_palette_query(&query),
            Event::PaletteMove(delta) => self.shell.move_palette_selection(delta),
            Event::Confirm(answer) => {
                let outcome = self.shell.answer_confirm(answer);
                self.report(outcome);
            }
            Event::GotoText(text) => self.shell.set_goto_text(&text),
            Event::GotoGo(text) => self.dispatch("nav.goto", &json!({ "text": text })),
            Event::WorkspaceText(text) => self.shell.set_workspace_text(&text),
            Event::FormField(name, text) => self.shell.set_form_field(name, &text),
            // A refusal stays in the dialog, so it is not made a banner as well.
            Event::FormSubmit => {
                self.shell.submit_form();
            }
            Event::DismissBanner => self.shell.dismiss_banner(),
            Event::DismissToast(serial) => self.shell.dismiss_toast(serial),
            Event::JunctionRow(row) => self.shell.set_junction_row(row),
            Event::OpenPrivacy => {
                if !self.platform.open_url(ew_viewmodel::about::PRIVACY_URL) {
                    let text = strings::format("privacy-open-failed", &[ew_viewmodel::about::PRIVACY_URL]);
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
                self.shell.show_banner(strings::text("banner-no-file-dialog"));
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
                self.shell.show_banner(strings::text("open-failed"));
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
                self.shell.show_banner(strings::text("doc-missing"));
            }
        }
    }
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
