// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The egui frame over the shell model (WP-S2.01 parts 1 and 2, spec section 5.3): the menu bar, the banner, the
//! start page, the first-run screen, the wizard, or the docked project view (the page list and the page canvas of
//! WP-S2.02), the status bar, the palette and the dialogs, and the routing of keyboard shortcuts.
//!
//! The frame paints from `ShellModel`'s getters and sends everything the user does to `ShellModel::dispatch`
//! (a menu item, a palette entry, a shortcut, a dialog answer, a file dialog's result). It names no `Session`.
//! `ew-ui` does not depend on eframe: the application binary calls [`App::ui`] from its `eframe::App`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use egui::{CentralPanel, Key, Modifiers, Panel, ThemePreference, Ui, ViewportCommand};
use ew_viewmodel::actions::Context;
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::shell::{ConfirmAnswer, Focus, Outcome, Prompt, PromptRequest, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::canvas::{CanvasCounters, CanvasHost};
use crate::canvas_area::CanvasAreaHost;
use crate::dock::Dock;
use crate::find::FindPane;
use crate::gpu::GpuHost;
use crate::keys::{self, Binding};
use crate::navigator::NavigatorPane;
use crate::page_view::PageView;
use crate::properties::PropertiesPane;
use crate::{about, first_run, menu, palette, start_page, status_bar, wizard};

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

/// An action that waits for one painted frame so that the status bar shows it running (the PDF export).
struct Deferred {
    action: &'static str,
    params: Value,
    /// Whether a frame with the label has been painted.
    painted: bool,
}

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
    /// The toast on show and when the window first painted it: it goes after 8 seconds measured by the window, never
    /// by the model (WP-S2.05 §6.6).
    toast_since: Option<(u64, std::time::Instant)>,
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
    dialog_was_open: bool,
    /// An action that waits one frame so that the status bar can show it running (the PDF export).
    deferred: Option<Deferred>,
    /// The file dialog that is open off the window's thread, with the request it answers (FT-467).
    pending_pick: Option<(PromptRequest, Receiver<Pick>)>,
    /// The `ui_scale` last given to egui: the zoom is set when the setting changes, not every frame, so that
    /// egui's own zoom keys keep working in between.
    applied_scale: Option<f32>,
    /// The theme last given to egui.
    applied_theme: Option<String>,
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
}

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
            toast_since: None,
            area_splitters: 0,
            preview_view: PageView::new(),
            dock,
            initial,
            bindings: keys::bindings(),
            title: String::new(),
            palette_was_open: false,
            picker_was_open: false,
            dialog_was_open: false,
            deferred: None,
            pending_pick: None,
            applied_scale: None,
            applied_theme: None,
            request_paste: false,
            ip_name: String::new(),
            drawing_fields_epoch: None,
            marker_csa: String::new(),
            marker_colours: String::new(),
            structure_caption: String::new(),
            structure_location: String::new(),
            text_buffer: None,
        }
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
    /// else none (tests: the handles are egui_tiles' own areas and have no accessible name, so the walker allows
    /// exactly this many; FT-338 names them).
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

    /// The panes of the dock tree in layout order (tests).
    pub fn dock_panes(&self) -> Vec<PaneKind> {
        self.dock.panes()
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
        if action == "edit.paste_clip"
            && params.get("text").is_none()
            && self.shell.paste_page_reason().is_none()
            && ew_viewmodel::actions::find(action).is_some_and(|a| (a.enabled)(&self.shell).is_ok())
        {
            self.request_paste = true;
            return;
        }
        let outcome = self.shell.dispatch(action, params);
        self.report(outcome);
    }

    /// A failure or incomplete Cut refusal becomes a banner when the shell has not shown one of its own.
    fn report(&mut self, outcome: Outcome) {
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
        let theme = self.shell.settings().theme.clone();
        if self.applied_theme.as_deref() != Some(theme.as_str()) {
            ctx.set_theme(match theme.as_str() {
                "light" => ThemePreference::Light,
                "dark" => ThemePreference::Dark,
                _ => ThemePreference::System,
            });
            self.applied_theme = Some(theme);
        }
        // The switch rule (WP-S2.02 section 7.2): the setting is effective at once, a failure moves the canvas to the
        // CPU with one banner, and the status bar shows the result.
        self.canvas.update_backend(&mut self.shell);
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.should_quit() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            // While a file dialog is open nothing the user does reaches the shell (see below): the window stays open.
            if self.pending_pick.is_none() {
                self.dispatch("app.quit", &Value::Null);
            }
        }
        // The export runs in the frame after the one that painted its label.
        if let Some(d) = self.deferred.as_mut() {
            if d.painted {
                if let Some(d) = self.deferred.take() {
                    self.dispatch(d.action, &d.params);
                }
            } else {
                d.painted = true;
                ctx.request_repaint();
            }
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

        let exporting = self.deferred.is_some();
        Panel::top("menu-bar").show(ui, |ui| menu::show(ui, &self.shell, &mut events));
        if let Some(banner) = self.shell.banner() {
            Panel::top("banner").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(banner);
                    if self.shell.can_discard_journal()
                        && ui.button(strings::text("action-file-discard-journal")).clicked()
                    {
                        events.push(Event::Action("file.discard_journal", Value::Null));
                    }
                    if ui.button(strings::text("banner-dismiss")).clicked() {
                        events.push(Event::DismissBanner);
                    }
                });
            });
        }
        Panel::bottom("status-bar").show(ui, |ui| status_bar::show(ui, &self.shell, exporting));
        let tool_epoch = self.shell.tool_epoch();
        if self.drawing_fields_epoch != Some(tool_epoch) {
            self.marker_csa.clear();
            self.marker_colours.clear();
            self.structure_caption.clear();
            self.structure_location.clear();
            self.drawing_fields_epoch = Some(tool_epoch);
        }
        // The tool strip (WP-S2.04 §6.4 item 4): the active tool's options as read-only text, above the canvas; shown
        // for every tool (Select's hint at rest), so its row never comes and goes during a gesture.
        if self.shell.has_project()
            && let Some(text) = crate::tool_view::tool_strip_text(&self.shell)
        {
            let ip_tool = self.shell.tool().name() == "ip";
            let marker_tool = self.shell.tool().name() == "marker";
            let structure_box_tool = self.shell.tool().name() == "structure_box";
            let externals = if ip_tool || structure_box_tool {
                self.shell.external_choices()
            } else {
                Vec::new()
            };
            let text_tool = self.shell.tool().name() == "text";
            let text_bind = if text_tool { self.shell.text_bind() } else { None };
            let ip_name = &mut self.ip_name;
            let marker_csa = &mut self.marker_csa;
            let marker_colours = &mut self.marker_colours;
            let structure_caption = &mut self.structure_caption;
            let structure_location = &mut self.structure_location;
            Panel::top("tool-strip").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(text);
                    // WP-S2.04 §9.1: the IP tool's name field and external-reference field.
                    if ip_tool {
                        crate::tool_view::ip_fields(ui, ip_name, &externals, &mut events);
                    }
                    // WP-S2.04 §10.5: the Text tool's binding.
                    if text_tool {
                        crate::tool_view::text_fields(ui, text_bind.as_deref(), &mut events);
                    }
                    // WP-S2.04 §10.5: the Wire marker's fields and the Structure box's style and fields.
                    if marker_tool {
                        crate::tool_view::marker_fields(ui, marker_csa, marker_colours, &mut events);
                    }
                    if structure_box_tool {
                        crate::tool_view::structure_box_fields(
                            ui,
                            &externals,
                            structure_caption,
                            structure_location,
                            &mut events,
                        );
                    }
                });
            });
        }
        let mut new_layout = None;
        let mut canvas_shown = false;
        let project_view =
            self.shell.first_run().is_none() && self.shell.wizard().is_none() && self.shell.has_project();
        if project_view {
            self.canvas.begin_frame(&ctx, &self.shell);
        }
        self.area_splitters = 0;
        CentralPanel::default().show(ui, |ui| {
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
                    ..
                } = self;
                navigators.retain(|k, _| shell.navigator_open(*k));
                dock.show(ui, &mut |ui, pane| match pane {
                    PaneKind::Pages => navigator.show(ui, shell, &mut events),
                    // WP-S2.06 part 1
                    PaneKind::Navigator(k) => navigators
                        .entry(k)
                        .or_insert_with(|| NavigatorPane::of(k))
                        .show(ui, shell, &mut events),
                    PaneKind::Properties => properties.show(ui, shell, &mut events),
                    PaneKind::Find => find.show(ui, shell, &mut events),
                    PaneKind::PageView => {
                        canvas_area.show(ui, shell, canvas, &mut events);
                        *area_splitters = canvas_area.splitter_count(shell);
                        canvas_shown = true;
                    }
                });
                new_layout = dock.layout().filter(|l| l != shell.layout());
            } else {
                start_page::show(ui, &self.shell, &mut events);
            }
        });
        let palette_open = self.shell.palette().is_some();
        palette::show(
            &ctx,
            &self.shell,
            palette_open && !self.palette_was_open,
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
        crate::junction::show(&ctx, &self.shell, &mut events);
        crate::text_popup::show(&ctx, &self.shell, &mut self.text_buffer, &mut events);
        crate::parts_card::show(&ctx, &self.shell, &mut events);
        let dialog_open = self.shell.dialog().is_some();
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
        // The toast's timeout is the window's (WP-S2.05 §6.6): 8 seconds after the frame that first showed it.
        let serial = self.shell.properties().toast().map(|t| t.serial);
        match (serial, self.toast_since) {
            (None, _) => self.toast_since = None,
            (Some(s), Some((t, since))) if s == t => {
                if ew_render2d::stats::now() - since >= Duration::from_secs(8) {
                    events.push(Event::Action("toast.dismiss", Value::Null));
                    self.toast_since = None;
                } else {
                    ctx.request_repaint_after(Duration::from_millis(500));
                }
            }
            (Some(s), _) => {
                self.toast_since = Some((s, ew_render2d::stats::now()));
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
        let dispatch_started = ew_render2d::stats::now();
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
                .end_frame(ew_render2d::stats::now() - dispatch_started, &self.shell);
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
            || self.shell.junction_editor().is_some()
            || self.shell.text_popup().is_some();
        let overlay = palette_open || dialog_open || picker_open;
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
            None => Context::Global,
        };
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
                let action = if binding.action == "find.replace_all" && replace_card {
                    "preview.apply"
                } else {
                    binding.action
                };
                events.push(Event::Action(action, Value::Null));
                return;
            }
        }
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
        if request.action == "file.export_pdf" {
            // The export is synchronous in part 1: one frame with the label on screen, then the export.
            self.deferred = Some(Deferred {
                action: request.action,
                params: Value::Object(params),
                painted: false,
            });
        } else {
            self.dispatch(request.action, &Value::Object(params));
        }
        self.prompt_answered(ctx);
    }

    /// The menu state after a dialog: close every popup and request the frame that paints the result.
    fn prompt_answered(&mut self, ctx: &egui::Context) {
        // The menu the dialog was opened from is closed in the frame that paints the answer.
        egui::Popup::close_all(ctx);
        ctx.request_repaint();
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
