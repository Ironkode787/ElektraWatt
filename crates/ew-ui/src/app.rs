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
use std::time::Duration;

use egui::{CentralPanel, Key, Modifiers, Panel, ScrollArea, ThemePreference, Ui, ViewportCommand};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::shell::{ConfirmAnswer, Outcome, Prompt, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::canvas::{CanvasCounters, CanvasHost};
use crate::dock::Dock;
use crate::keys::{self, Binding};
use crate::page_view::PageView;
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
    /// The go-to-page field changed.
    GotoText(String),
    /// Go in the go-to-page dialog.
    GotoGo(String),
    /// The save-layout field changed.
    WorkspaceText(String),
    /// The banner's own button.
    DismissBanner,
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

/// The operating system's file dialogs and browser, which the application binary provides (`rfd` and the system's
/// opener) and tests replace.
pub trait Platform {
    /// Asks the user for a path: a project directory, a new directory, a PDF file or a QElectroTech file.
    fn pick(&mut self, prompt: Prompt) -> Pick;

    /// Opens a `file://` URL of the installed user guide in the system browser (a user action; no network traffic);
    /// `false` when it could not be opened.
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
    /// The page canvas of the project view (WP-S2.02).
    canvas: CanvasHost,
    /// The page view of the wizard's preview.
    preview_view: PageView,
    dock: Dock,
    initial: Option<PathBuf>,
    bindings: Vec<Binding>,
    title: String,
    palette_was_open: bool,
    dialog_was_open: bool,
    last_page_index: Option<usize>,
    /// An action that waits one frame so that the status bar can show it running (the PDF export).
    deferred: Option<Deferred>,
    /// The `ui_scale` last given to egui: the zoom is set when the setting changes, not every frame, so that
    /// egui's own zoom keys keep working in between.
    applied_scale: Option<f32>,
    /// The theme last given to egui.
    applied_theme: Option<String>,
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
            preview_view: PageView::new(),
            dock,
            initial,
            bindings: keys::bindings(),
            title: String::new(),
            palette_was_open: false,
            dialog_was_open: false,
            last_page_index: None,
            deferred: None,
            applied_scale: None,
            applied_theme: None,
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

    /// The engine counters of the canvas (tests: tiles requested, rastered, uploaded; scenes built).
    pub fn canvas_counters(&self) -> CanvasCounters {
        self.canvas.counters()
    }

    /// The frame statistics of the canvas (WP-S2.02 section 9).
    pub fn canvas_stats(&self) -> &ew_render2d::stats::FrameStats {
        self.canvas.stats()
    }

    /// Whether the image of the wizard's preview has arrived from the render worker.
    pub fn preview_ready(&self) -> bool {
        self.preview_view.is_ready()
    }

    /// The number of splitter handles on screen: the dock's while the project view shows, else none (tests: the
    /// handles are egui_tiles' own areas and have no accessible name, so the walker allows exactly this many).
    #[doc(hidden)]
    pub fn splitter_count(&self) -> usize {
        if self.shell.first_run().is_none() && self.shell.wizard().is_none() && self.shell.has_project() {
            self.dock.splitters()
        } else {
            0
        }
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
        let outcome = self.shell.dispatch(action, params);
        self.report(outcome);
    }

    /// A failure the shell has not put into a banner of its own becomes one.
    fn report(&mut self, outcome: Outcome) {
        if let Outcome::Failed { message } = outcome
            && self.shell.banner().is_none()
        {
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
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.should_quit() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dispatch("app.quit", &Value::Null);
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
        if self.dock.version() != self.shell.layout_version() {
            self.dock = Dock::new(self.shell.layout(), self.shell.layout_version());
        }
        // The preview is built from the draft before the frame reads it (a no-op while it is current).
        self.shell.wizard_prepare_preview();

        let mut events: Vec<Event> = Vec::new();
        self.route_keys(&ctx, &mut events);

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
        let mut new_layout = None;
        let mut canvas_shown = false;
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
                    last_page_index,
                    ..
                } = self;
                dock.show(ui, &mut |ui, pane| match pane {
                    PaneKind::Pages => page_list(ui, shell, last_page_index, &mut events),
                    PaneKind::PageView => {
                        canvas.show(ui, shell, &mut events);
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

        if !events.is_empty() {
            ctx.request_repaint();
        }
        let dispatch_started = ew_render2d::stats::now();
        for event in events {
            self.apply(event);
        }
        if canvas_shown {
            // The canvas's input phase includes dispatching the frame's actions (WP-S2.02 section 9).
            self.canvas
                .end_frame(ew_render2d::stats::now() - dispatch_started, &self.shell);
        }
        self.answer_prompts(&ctx);
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
    /// shortcuts; Escape closes the topmost popup or dialog and otherwise does nothing. F1 is handled at the end of
    /// the frame, in every state.
    fn route_keys(&mut self, ctx: &egui::Context, events: &mut Vec<Event>) {
        let palette_open = self.shell.palette().is_some();
        let dialog_open = self.shell.dialog().is_some();
        let overlay = palette_open || dialog_open;
        if overlay && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            events.push(Event::Action("ui.dismiss", Value::Null));
            return;
        }
        if dialog_open || (ctx.text_edit_focused() && !palette_open) {
            return;
        }
        for binding in &self.bindings {
            if matches!(binding.action, "ui.dismiss" | "help.documentation")
                || (palette_open && binding.action != "tools.palette")
            {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&binding.shortcut)) {
                events.push(Event::Action(binding.action, Value::Null));
                break;
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
            Event::GotoGo(folio) => self.dispatch("page.goto", &json!({ "folio": folio })),
            Event::WorkspaceText(text) => self.shell.set_workspace_text(&text),
            Event::DismissBanner => self.shell.dismiss_banner(),
        }
    }

    /// The file dialogs the shell asked for: their answers are dispatched like any other input.
    fn answer_prompts(&mut self, ctx: &egui::Context) {
        while let Some(request) = self.shell.take_prompt() {
            let path = match self.platform.pick(request.prompt) {
                Pick::Path(path) => path,
                Pick::Cancelled => continue,
                Pick::Unavailable => {
                    self.shell.show_banner(strings::text("banner-no-file-dialog"));
                    continue;
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
                ctx.request_repaint();
            } else {
                self.dispatch(request.action, &Value::Object(params));
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

/// The page list pane: the printed folio labels, the current one selected; a click is `page.goto`.
fn page_list(ui: &mut Ui, shell: &ShellModel, last_index: &mut Option<usize>, events: &mut Vec<Event>) {
    let list = ew_viewmodel::page_list::PageList::of(shell);
    let moved = *last_index != list.current;
    *last_index = list.current;
    ScrollArea::vertical().show(ui, |ui| {
        for (i, label) in list.labels.iter().enumerate() {
            let selected = list.current == Some(i);
            let response = ui.selectable_label(selected, label.as_str());
            if selected && moved {
                response.scroll_to_me(None);
            }
            if response.clicked() {
                events.push(Event::Action("page.goto", json!({ "folio": label })));
            }
        }
    });
}
