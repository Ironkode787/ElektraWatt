// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The frame's events applied, the file dialogs started and answered, and the platform's requests (the guide, a
//! written file, a second window, the log folder, a print).

use super::*;

impl App {
    pub(super) fn apply(&mut self, event: Event) {
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
            Event::SymbolAsk(action, params) => {
                self.run(None, |shell| shell.ask_symbol_discard(action, params))
            }
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
            Event::DisabledClick(action, params) => self.shell.explain_disabled(action, &params),
            Event::JunctionRow(row) => self.shell.set_junction_row(row),
            // The bundled copy when there is one, else the repository's.
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
    /// dialogs run off the window's thread, so a dialog's answer is read here, at the start of a frame, once
    /// its thread has sent it; while it is open the shell's next prompt waits. A platform that answers at once (the
    /// tests') is answered in the same frame.
    pub(super) fn answer_prompts(&mut self, ctx: &egui::Context) {
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
        // The PDF export is a job now (its request carries `replace`: the save dialog asked).
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

    /// Open PDF: the written file in the system's viewer; a banner when it could not be opened.
    pub(super) fn open_results(&mut self) {
        while let Some(file) = self.shell.take_open_request() {
            if !self.platform.open_file(&file) {
                self.shell.show_banner(strings::text(key!("open-failed")));
            }
        }
    }

    /// A tutorial's practice project in a second window; a banner when it could not be started (the project is in the
    /// recent list).
    pub(super) fn open_windows(&mut self) {
        while let Some(project) = self.shell.take_window_request() {
            if !self.platform.open_window(&project) {
                self.shell
                    .show_banner(strings::text(key!("banner-second-window-failed")));
            }
        }
    }

    /// The start-up offer's Open log folder: the folder in the system's file manager; the offer says
    /// where it is when it could not be opened (S7).
    pub(super) fn open_log_folder(&mut self) {
        if let Some((url, folder)) = self.shell.take_log_folder_request()
            && !self.platform.open_url(&url)
        {
            self.shell.log_folder_failed(&folder);
        }
    }

    /// The print pre-dialog's Print… and Open in PDF viewer: the ends that are in, each
    /// with its one toast, then the new requests handed to the platform.
    pub(super) fn print_requests(&mut self, ctx: &egui::Context) {
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
    pub(super) fn open_documentation(&mut self) {
        while let Some(request) = self.shell.take_doc_request() {
            let opened = request
                .url
                .as_deref()
                .is_some_and(|url| self.platform.open_url(url));
            if opened {
                continue;
            }
            // An offscreen visual run names the page's file, so that a reader can read the page it was sent to.
            match self.platform.guide_file(&request.target) {
                Some(file) => self.shell.note_guide_page(&file),
                None => self.shell.show_banner(strings::text(key!("doc-missing"))),
            }
        }
    }
}
