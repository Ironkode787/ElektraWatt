// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One frame of the window ([`App::ui`]): the settings it follows, the periodic tasks, the input, the chrome, the
//! screen, what floats over the page, the popups, what is said and asked, then the frame's actions.

use super::*;

impl App {
    /// The banner of doc 16a §6.11 as an overlay: along the top edge of the canvas below the
    /// page tabs when a page view is shown, else under the command bar, full width of that area. Showing or dismissing
    /// it never moves the canvas or a panel. Returns its
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
        let frame = crate::components::band_frame(tokens, tokens.warning_subtle);
        let mut buttons = Vec::new();
        if self.shell.can_discard_journal() {
            buttons.push((
                crate::components::Kind::Primary,
                strings::text(key!("action-file-discard-journal")),
                Event::Action("file.discard_journal", Value::Null),
            ));
        }
        // Doc 16a §6.11: the held-saves banner's one Primary reopens the choice; a message over
        // it (a refused save) keeps it.
        if self.shell.external_choice_waiting() {
            buttons.push((
                crate::components::Kind::Primary,
                strings::text(key!("banner-external-choose")),
                Event::Action("external.review", Value::Null),
            ));
        }
        // The damaged-copy banner's one Primary updates the copies from their
        // libraries; a banner of a copy no library holds offers nothing.
        if self.shell.damaged_update_offered() {
            buttons.push((
                crate::components::Kind::Primary,
                strings::text(key!("action-library-update-damaged")),
                Event::Action("library.update_damaged", Value::Null),
            ));
        }
        if self.shell.banner_dismissable() {
            buttons.push((
                crate::components::Kind::Subtle,
                strings::text(key!("banner-dismiss")),
                Event::DismissBanner,
            ));
        }
        let inner_width = (area.width() - 2.0 * tokens.space_4).max(0.0);
        let response = egui::Area::new(egui::Id::unique("banner"))
            .fixed_pos(area.min)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.set_clip_rect(area);
                frame.show(ui, |ui| {
                    ui.set_width(inner_width);
                    ui.horizontal_wrapped(|ui| {
                        crate::components::band_row(
                            ui,
                            tokens,
                            (crate::icons::IconId::alert_triangle, tokens.warning_icon),
                            egui::RichText::new(banner).color(tokens.text_primary.color32()),
                            buttons,
                            events,
                        );
                    });
                });
            });
        Some(response.response.rect)
    }

    /// Paints one frame into the root `ui` of the window: the settings it follows, the periodic tasks, the input,
    /// the chrome, the screen, what floats over the page, the popups, what is said and asked, then the frame's
    /// actions and the platform's requests.
    pub fn ui(&mut self, ui: &mut Ui) {
        let _frame = tracing::trace_span!("frame").entered();
        let ctx = ui.ctx().clone();
        // The window's parts read the clock it runs on from the frame's context.
        self.clock.publish(&ctx);
        self.apply_settings(&ctx);
        let (now, hide_job) = self.tick(&ctx);

        let mut events: Vec<Event> = Vec::new();
        self.properties.begin_frame();
        let (form_key, held_for_picker) = self.input(&ctx, &mut events);
        // The chrome paints `color.surface.chrome` itself (doc 16a §4.14); panels take `color.surface.panel`.
        let tokens = crate::theme::current(&ctx);
        self.chrome_panels(ui, &ctx, &tokens, hide_job, &mut events);
        self.follow_tool_epoch();
        let (page_pane, hud_strip, canvas_shown, new_layout) =
            self.central(ui, &ctx, &tokens, now, &mut events);
        self.over_page(&ctx, (page_pane, hud_strip, canvas_shown), &mut events);
        self.popups(&ctx, held_for_picker, &mut events);
        self.said_and_asked(&ctx, &tokens, form_key, &mut events);
        self.dispatch_events(&ctx, &tokens, canvas_shown, events);
        self.platform_requests(&ctx);
        self.end_frame(&ctx, new_layout);
    }

    /// The settings the frame follows: the key bindings, the UI scale, the theme with the density, the minimum text
    /// size and reduced motion.
    fn apply_settings(&mut self, ctx: &egui::Context) {
        // `ui_scale` is a factor on the native scale: egui's zoom factor is exactly that, and follows the window
        // to another monitor. It is set once when the setting changes.
        // The bindings follow the keymap preset and the user's keymap.
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
        // The theme and the density as tokens (doc 16a §4.14), applied when either setting changes;
        // `system` lets egui follow the operating system between Light and Dark.
        // The minimum text size and reduced motion with them.
        let appearance = (
            self.shell.settings().theme.clone(),
            self.shell.settings().density.clone(),
            self.shell.settings().min_font_pt,
            self.shell.reduce_motion(),
        );
        if self.applied_theme.as_ref() != Some(&appearance) {
            crate::theme::apply_setting(&appearance.0, &appearance.1, appearance.2, appearance.3, ctx);
            // A visual run draws the caret always, so a frame never catches it off.
            if self.clock.is_frame() {
                ctx.all_styles_mut(|s| s.visuals.text_cursor.blink = false);
            }
            self.applied_theme = Some(appearance);
        }
        // Reduced motion ends egui's wheel smoothing too.
        if self.shell.reduce_motion() {
            crate::theme::unsmoothed_scroll(ctx);
        }
    }

    /// The periodic tasks of a frame: the canvas's backend, the window's geometry, a close request, the focus coming
    /// back, the app menu's keys, the jobs, the New project dialog's folders, the project on disk, the project to
    /// open first, the file dialogs' answers, the dock's layout and the wizard's preview. Returns the frame time and
    /// whether the status bar hides its job field (the 200 ms rule).
    fn tick(&mut self, ctx: &egui::Context) -> (f64, bool) {
        // The switch rule (section 7.2): the setting is effective at once, a failure moves the canvas to the
        // CPU with one banner, and the status bar shows the result.
        self.canvas.update_backend(&mut self.shell);
        // The window's geometry, written at the end of the window.
        crate::window_memory::remember(ctx, &mut self.shell, &mut self.window_zoom);
        if ctx.input(|i| i.viewport().close_requested()) && !self.shell.should_quit() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            // While a file dialog is open nothing the user does reaches the shell (see below): the window stays open.
            if self.pending_pick.is_none() {
                self.dispatch("app.quit", &Value::Null);
            }
        }
        // A native dialog closed, or the user came back from another window: the window regains the focus
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
        // Alt pressed and released alone opens the app menu (doc 16a §5.2), and the actions only the window runs go.
        self.app_menu_keys(ctx);
        self.window_actions(ctx);
        // The jobs wake the window after each progress report and at their end, and every frame takes
        // the ends that are in (no `Session` call).
        // On the frame clock (a visual run) the driver waits for the jobs itself: no job thread wakes the frame, so
        // which frame sees an end never depends on timing.
        if !self.waker_set && !self.clock.is_frame() {
            let waker = ctx.clone();
            self.shell
                .set_job_waker(Some(std::sync::Arc::new(move || waker.request_repaint())));
            self.waker_set = true;
        }
        self.shell.poll_jobs();
        let now = ctx.input(|i| i.time);
        // The New project dialog reads its folders again when the window regains the focus and once a second, so
        // that a folder made outside meanwhile is seen; not every frame.
        if matches!(
            self.shell.dialog(),
            Some(ew_viewmodel::shell::Dialog::NewProject(_))
        ) {
            if regained || now - self.new_project_checked >= 1.0 {
                self.shell.recheck_new_project();
                self.new_project_checked = now;
            }
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        // The project on disk is polled every 3 s, 750 ms later again while a change settles,
        // and when the window regains the focus; never while a pointer button is held (a gesture) or on the frame clock
        // (a visual run polls through `project.check_disk`).
        if self.shell.has_project() && !self.clock.is_frame() {
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
        // is in the shell before this frame paints (W3); an answer from a dialog's thread starts this frame.
        self.answer_prompts(ctx);
        if self.dock.version() != self.shell.layout_version() {
            self.dock = Dock::new(self.shell.layout(), self.shell.layout_version());
        }
        // The preview is built from the draft before the frame reads it (a no-op while it is current).
        self.shell.wizard_prepare_preview();
        (now, hide_job)
    }

    /// The frame's input before the panes paint: a form dialog's Ctrl+Enter (returned), the shortcuts, the keys held
    /// back for the open picker (returned), a dropped image.
    fn input(&mut self, ctx: &egui::Context, events: &mut Vec<Event>) -> (bool, Vec<egui::Event>) {
        // Ctrl+Enter submits a form dialog from any focus. The form takes the key before the keymap
        // and the panes under it, so their Ctrl+Enter (`canvas.activate_alt`, `preview.apply`, `find.replace_all`)
        // does not fire.
        let form_key = matches!(self.shell.dialog(), Some(ew_viewmodel::shell::Dialog::Form(_)))
            && ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        self.route_keys(ctx, events);
        // While the picker is open it owns the keyboard. The panes paint before it and
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
        (form_key, held_for_picker)
    }

    /// The chrome around the screen: the command bar, the template band, the status bar.
    fn chrome_panels(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tokens: &crate::theme::Tokens,
        hide_job: bool,
        events: &mut Vec<Event>,
    ) {
        let chrome = tokens.surface_chrome.color32();
        // The command bar replaces the menu bar (doc 16a §5.2); the app button opens the app menu.
        let bar_height =
            crate::command_bar::bar_height(tokens, ew_viewmodel::command_bar::shown_collapsed(&self.shell));
        let command_bar = &mut self.command_bar;
        let shell = &self.shell;
        Panel::top("command-bar")
            .exact_size(bar_height)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(chrome))
            .show(ui, |ui| {
                command_bar.show(ui, shell, events, &mut self.symbol_editor_view)
            });
        // An open template says so in a band across the window, under the command bar: it cannot be mistaken for a
        // project. The band takes its own room, so it covers no drawing.
        let template = matches!(self.shell.screen(), Screen::Project | Screen::Start)
            .then(|| self.shell.editing_template())
            .flatten();
        if let Some(name) = template {
            Panel::top("template-band")
                .resizable(false)
                .show_separator_line(false)
                .frame(crate::components::band_frame(tokens, tokens.accent_subtle))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        crate::components::band_row(
                            ui,
                            tokens,
                            (crate::icons::IconId::file_stack, tokens.accent),
                            crate::components::body_strong(
                                tokens,
                                strings::format(key!("template-band"), &[&name]),
                            ),
                            vec![(
                                crate::components::Kind::Secondary,
                                strings::text(key!("template-band-close")),
                                Event::Action("file.close", Value::Null),
                            )],
                            events,
                        );
                    });
                });
        }
        // F6 reached the command bar: its active tab takes egui's keyboard focus.
        let bar_has_focus = self.shell.focus() == Some(Focus::CommandBar);
        if crate::components::opened(
            ctx,
            egui::Id::unique("command-bar-focus"),
            bar_has_focus.then_some(()),
        ) {
            let tab = crate::command_bar::tab_id(self.shell.command_tab());
            ctx.memory_mut(|m| m.request_focus(tab));
        }
        // `size.status` high (doc 16a §5.8).
        Panel::bottom("status-bar")
            .exact_size(tokens.size_status)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.fill(chrome))
            .show(ui, |ui| status_bar::show(ui, &self.shell, hide_job, events));
    }

    /// The drawing tools' option fields start empty with each new tool.
    fn follow_tool_epoch(&mut self) {
        let tool_epoch = self.shell.tool_epoch();
        if self.drawing_fields_epoch != Some(tool_epoch) {
            self.marker_csa.clear();
            self.marker_colours.clear();
            self.structure_caption.clear();
            self.structure_location.clear();
            self.drawing_fields_epoch = Some(tool_epoch);
        }
    }

    /// The screen in the central panel (the first run, the wizard, the symbol editor, the docked project view with
    /// its rails, or the start page). Returns the page view pane and the HUD's strip on it, whether the canvas was
    /// shown, and the dock's layout when the user changed it.
    fn central(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        tokens: &crate::theme::Tokens,
        now: f64,
        events: &mut Vec<Event>,
    ) -> (
        Option<egui::Rect>,
        Option<egui::Rect>,
        bool,
        Option<ew_viewmodel::layout::Layout>,
    ) {
        let mut new_layout = None;
        let mut canvas_shown = false;
        let screen = self.shell.screen();
        if screen == Screen::Project {
            self.canvas.begin_frame(ctx);
        }
        self.area_splitters = 0;
        if screen == Screen::Project {
            let chrome = tokens.surface_chrome.color32();
            // Doc 16a §5.4, §5.6: the navigator rail at the left, the palettes rail at the right.
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
                            crate::rail::show_left(ui, &self.shell, events);
                        } else {
                            crate::rail::show_right(ui, &self.shell, events);
                        }
                    });
            }
        }
        let mut page_pane = None;
        let mut hud_strip = None;
        if self.shell.symbol_editor().is_none() {
            self.symbol_editor_view = crate::symbol_editor::EditorView::default();
        }
        if self.shell.parts_place().is_none() {
            self.parts_view = crate::parts_place::PlaceView::default();
        }
        CentralPanel::default().show(ui, |ui| {
            crate::properties::remember_free_area(ui);
            match screen {
                Screen::FirstRun => first_run::show(ui, &self.shell, events),
                Screen::Wizard => wizard::show(ui, &self.shell, &mut self.preview_view, events),
                // The drawing takes the middle of Parts while it shows (doc 07 §5.6).
                Screen::Parts if self.shell.symbol_editor().is_some() => {
                    self.symbol_editor_view.show(ui, &self.shell, events);
                }
                Screen::Parts => self.parts_view.show(ui, &self.shell, events),
                Screen::Project => {
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
                    // Doc 16a §5.7: the Messages tab's count chip shows the open findings (the status bar's rule).
                    let counts = [(
                        PaneKind::Navigator(ew_viewmodel::navigators::NavKind::Messages),
                        shell.messages_badge(),
                    )];
                    // The Preview tab's dot while a card waits for Apply.
                    let marks: Vec<(PaneKind, String)> = ew_viewmodel::status::waiting_text(shell)
                        .map(|words| (PaneKind::Preview, words))
                        .into_iter()
                        .collect();
                    let headers = dock.show(ui, collapsed, &counts, &marks, &mut |ui, pane| match pane {
                        PaneKind::Pages => navigator.show(ui, shell, events),
                        PaneKind::Navigator(k) => navigators
                            .entry(k)
                            .or_insert_with(|| NavigatorPane::of(k))
                            .show(ui, shell, events),
                        PaneKind::Properties => properties.show(ui, shell, events),
                        PaneKind::Find => find.show(ui, shell, events),
                        PaneKind::Jobs => crate::jobs::show(ui, shell, job_started, now, events),
                        PaneKind::Preview => crate::preview::show(ui, shell, events),
                        PaneKind::History => crate::history::show(ui, shell, events),
                        PaneKind::Glossary => crate::glossary::show(ui, shell, events),
                        PaneKind::PageView => {
                            // The canvas takes the whole pane (doc 16a §5.5): the HUD, the zoom control and the tool
                            // options float over it, each its own rounded shape with a shadow and nothing behind it
                            // (a strip under the HUD cut the drawing off). A station's view takes the whole pane too;
                            // the HUD, the zoom control and the tool options stay away from it (below).
                            let full = ui.max_rect();
                            // The HUD stands above the page's horizontal scroll bar while one shows.
                            let bar = (0..crate::canvas::GROUPS)
                                .filter_map(|g| shell.canvas_of(g))
                                .any(|c| crate::canvas::shows_scroll_bar(c, false));
                            let foot = full.bottom() - if bar { tokens.space_3 } else { 0.0 };
                            let split = (foot - crate::hud::strip_height(tokens)).max(full.top());
                            page_pane = Some(full);
                            hud_strip = Some(egui::Rect::from_min_max(
                                egui::pos2(full.left(), split),
                                egui::pos2(full.right(), foot.max(split)),
                            ));
                            canvas_area.show(ui, shell, canvas, events);
                            *area_splitters = canvas_area.splitter_count(shell);
                            canvas_shown = true;
                        }
                    });
                    events.extend(headers);
                    new_layout = dock.layout().filter(|l| l != shell.layout());
                }
                Screen::Start => start_page::show(ui, &self.shell, events),
            }
        });
        (page_pane, hud_strip, canvas_shown, new_layout)
    }

    /// What floats over the page view: the HUD, the zoom control and the tool options, kept clear by the canvas's
    /// chips; `page` is the page view pane, the HUD's strip on it and whether the canvas was shown.
    fn over_page(
        &mut self,
        ctx: &egui::Context,
        (page_pane, hud_strip, canvas_shown): (Option<egui::Rect>, Option<egui::Rect>, bool),
        events: &mut Vec<Event>,
    ) {
        // The HUD, the zoom control and the tool options over the page view (doc 16a §5.5).
        // A station's view is no page: the HUD, the zoom control and the tool options stay away from it.
        let station = self.shell.station_tab().is_some();
        self.station_pane = page_pane.filter(|_| canvas_shown && station);
        self.page_pane = page_pane.filter(|_| canvas_shown && !station);
        self.hud_rect = None;
        let mut options_card = None;
        if self.page_pane.is_none() {
            self.options_rect = None;
        }
        if let Some(pane) = self.page_pane {
            if let Some(strip) = hud_strip {
                self.hud_rect = Some(crate::hud::show(ctx, &self.shell, strip, events));
            }
            crate::hud::show_zoom(ctx, &self.shell, pane, events);
            options_card = self.tool_options(ctx, pane, events);
        }
        // OV2/OV6: the canvas painted before the HUD (whose place follows the pointer), so the chips keep off this
        // frame's HUD, zoom control and tool options card from the next frame on; the next input repaints
        // anyway, no extra repaint.
        let chrome: Vec<egui::Rect> = match (self.page_pane, self.hud_rect) {
            (Some(pane), Some(hud)) => [hud, crate::hud::zoom_rect(&crate::theme::current(ctx), pane)]
                .into_iter()
                // No obstacle for a ghost under the card.
                .chain(options_card.filter(|(_, faded)| !faded).map(|(rect, _)| rect))
                .collect(),
            _ => Vec::new(),
        };
        for group in 0..crate::canvas::GROUPS {
            self.canvas.set_chrome_obstacles(group, chrome.clone());
        }
    }

    /// The popups: the palette, the picker (given the keys held back for it), the Join chooser, the junction editor,
    /// the text popup, the parts card, the parts import's mapping and Show changes.
    fn popups(&mut self, ctx: &egui::Context, held_for_picker: Vec<egui::Event>, events: &mut Vec<Event>) {
        // The palette, the picker and a dialog take their first focus in the frame they open (the picker's when it
        // moves on to a part's symbols too; a dialog's when it replaced the last in one frame, the collision dialog
        // after Renumber, or is of the same kind, rebuilt by a second change: [`ShellModel::dialog_generation`]).
        let palette_open = self.shell.palette().is_some();
        palette::show(
            ctx,
            &self.shell,
            crate::components::opened(
                ctx,
                egui::Id::unique("palette-opened"),
                palette_open.then_some(()),
            ),
            self.command_bar.search_rect(),
            events,
        );
        let picker_step = self.shell.picker().map(|p| p.part);
        if !held_for_picker.is_empty() {
            ctx.input_mut(|i| i.events.extend(held_for_picker));
        }
        crate::picker::show(
            ctx,
            &self.shell,
            crate::components::opened(ctx, egui::Id::unique("picker-opened"), picker_step),
            events,
        );
        // The Place tool's Join chooser (doc 16a §6.22).
        crate::chooser::show(ctx, &self.shell, self.page_pane, &mut self.chooser_typing, events);
        crate::junction::show(ctx, &self.shell, events);
        crate::text_popup::show(ctx, &self.shell, &mut self.text_buffer, events);
        crate::parts_card::show(ctx, &self.shell, events);
        // The parts import's column mapping step.
        crate::parts_map::show(ctx, &self.shell, events);
        // Show changes of the history panel.
        crate::history::show_changes(ctx, &self.shell, events);
    }

    /// What is said and asked: the banner, the toasts, the notifications log, the dialog (`form_key`: a form's
    /// Ctrl+Enter), the start-up offer, and F1 with what is under the pointer.
    fn said_and_asked(
        &mut self,
        ctx: &egui::Context,
        tokens: &crate::theme::Tokens,
        form_key: bool,
        events: &mut Vec<Event>,
    ) {
        // The toasts and the notifications log.
        // At the canvas's top right (doc 16a §6.10).
        let bar_bottom = self.command_bar.bar_rect().map_or(0.0, |r| r.bottom());
        let banner = self.banner_overlay(ctx, tokens, bar_bottom, events);
        let place = crate::notifications::ToastPlace {
            page_pane: self.page_pane,
            station_pane: self.station_pane,
            bar_bottom,
            banner,
            corner: self.symbol_editor_view.toast_corner,
        };
        self.toasts.show(ctx, &self.shell, tokens, &place, events);
        crate::notifications::show_log(ctx, &self.shell, events);
        let opening = self.shell.dialog().map(|_| self.shell.dialog_generation());
        about::show(
            ctx,
            &self.shell,
            crate::components::opened(ctx, egui::Id::unique("dialog-opened"), opening),
            form_key,
            events,
        );
        // The start-up offer, while no other dialog is open.
        if self.shell.dialog().is_none() {
            crate::startup::show(ctx, &self.shell, events);
        }
        // What the window names under the pointer goes to the shell, where the status bar says what F1 opens. The
        // keys of `help.documentation` (F1 unless the keymap binds another) open it, else the thing on the page under
        // the pointer, else the context (the focus, the selection, the tool, the screen), in every state.
        self.shell.set_help_under(crate::help::take(ctx));
        let help = (self.bindings.iter())
            .filter(|b| b.action == "help.documentation")
            .any(|b| ctx.input_mut(|i| crate::keys::take_chord(i, &b.shortcut)));
        if help {
            let params = ew_viewmodel::help::pointer_topic(&self.shell)
                .map_or(Value::Null, |under| serde_json::json!({ "topic": under.topic }));
            events.push(Event::Action("help.documentation", params));
        }
    }

    /// The frame's actions after it painted: the canvas's tile submission, the toasts' timeouts, then each event,
    /// the clipboard's requests and the repaints a change asks for; the canvas keeps, hides or releases its tiles.
    fn dispatch_events(
        &mut self,
        ctx: &egui::Context,
        tokens: &crate::theme::Tokens,
        canvas_shown: bool,
        mut events: Vec<Event>,
    ) {
        if canvas_shown {
            // The merged tile submission and the hidden-first eviction of every canvas.
            self.canvas.finish_frame(ctx, &self.shell);
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
        self.toasts.expire(ctx, &self.shell, tokens, &mut events);
        let dispatch_started = self.clock.now();
        // The window repaints after an action that changed the model or the tool's marks without pointer motion
        // (an undo, a key-driven commit, a gesture's release, which painted nothing new before): once at once, and once
        // shortly after, when the page's new tiles have had time to arrive.
        let marks_before = (self.shell.revision(), self.shell.tool_overlay().len());
        for event in events {
            // `ui.dismiss` while the app menu is open closes the menu and nothing else, as
            // Escape does; so does any menu, a context menu among them.
            if matches!(event, Event::Action("ui.dismiss", _)) && menu_takes_dismiss(ctx) {
                continue;
            }
            self.apply(event);
        }
        // A menu a key of this frame asked for opens in the next frame.
        if !self.window_actions.is_empty() {
            ctx.request_repaint();
        }
        // What a copy wrote goes to the system clipboard (egui's `copy_text`).
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
            // The canvas's input phase includes dispatching the frame's actions (section 9).
            self.canvas.end_frame(self.clock.now() - dispatch_started);
        } else if self.shell.has_project()
            && !matches!(self.shell.screen(), Screen::FirstRun | Screen::Wizard)
        {
            // The page pane is behind another tab of the dock: its tiles stay within a share of the cache.
            self.canvas.hide();
        } else {
            // No canvas on screen (start page, wizard, first run): its tiles and queued jobs go.
            self.canvas.release();
        }
    }

    /// The platform's part of the frame's requests: the guide, a written file, a second window, the log folder, a
    /// print.
    fn platform_requests(&mut self, ctx: &egui::Context) {
        self.open_documentation();
        self.open_results();
        self.open_windows();
        self.open_log_folder();
        self.print_requests(ctx);
    }

    /// The end of a frame: the dock's layout given to the shell and written when due, the window's title, the close.
    fn end_frame(&mut self, ctx: &egui::Context, new_layout: Option<ew_viewmodel::layout::Layout>) {
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
}

/// A job younger than this shows no status-bar field.
const JOB_FIELD_AFTER: Duration = Duration::from_millis(200);

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

/// The first of the dropped files, which the window's one drop flow, the Image tool (doc 16 §4.8), takes: a folder is
/// none. A file the asset store does not accept is refused by `image.file` with the store's text.
pub(crate) fn dropped_image(files: &[PathBuf]) -> Option<&PathBuf> {
    files.iter().find(|f| !f.is_dir())
}

#[cfg(test)]
mod drop_tests {
    use super::*;

    /// The first dropped file goes to the Image tool, whatever its kind (the store says why it takes no other).
    #[test]
    fn the_first_dropped_file_goes_to_the_image_tool() {
        let p = |s: &str| PathBuf::from(s);
        let files = [p("plant.QET"), p("logo.svg")];
        assert_eq!(dropped_image(&files), Some(&files[0]));
        assert_eq!(dropped_image(&[p("noext")]), Some(&p("noext")));
        assert_eq!(dropped_image(&[]), None);
    }
}
