// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start page (WP-S2.01 parts 1 and 2; UX/UI doc 16 section 3.7): New project from the basic project (the
//! default one first), Open, the recent projects, Open sample project, Import QElectroTech project, Create company
//! standard, Tutorials, the quick switches for language and keymap, and the privacy line "Update check: off ·
//! change". Nothing on the page opens a connection (NFR-PRIV-01). It appears before any project is touched; no work
//! that could take time runs before the first frame.

use egui::Ui;
use ew_viewmodel::settings::{KEYMAPS, LANGUAGES};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

fn action(events: &mut Vec<Event>, id: &'static str, params: Value) {
    events.push(Event::Action(id, params));
}

/// A row of choices for one setting: the label, then one selectable button per option.
fn quick_switch(
    ui: &mut Ui,
    events: &mut Vec<Event>,
    label_key: &str,
    key: &str,
    current: &str,
    options: &[&str],
) {
    ui.horizontal(|ui| {
        ui.label(strings::text(label_key));
        for option in options {
            let text = strings::text(&format!("{key}-{option}"));
            if ui.selectable_label(*option == current, text).clicked() && *option != current {
                action(events, "settings.set", json!({ "key": key, "value": option }));
            }
        }
    });
}

/// Paints the start page into `ui`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.08).min(60.0));
            ui.heading(strings::text("start-title"));
            ui.add_space(16.0);
            if ui.button(strings::text("start-new")).clicked() {
                action(events, "file.new", Value::Null);
            }
            ui.weak(match shell.default_template_name() {
                Some(name) => strings::format("start-template", &[&name]),
                None => strings::text("start-template-builtin").to_owned(),
            });
            ui.add_space(8.0);
            if ui.button(strings::text("start-open")).clicked() {
                action(events, "file.open", Value::Null);
            }
            let names = shell.recent_names();
            if !names.is_empty() {
                ui.add_space(12.0);
                ui.label(strings::text("start-recent"));
                for (i, name) in names.iter().enumerate() {
                    let response = ui.button(name.as_str());
                    let response = match shell.recent_paths().get(i) {
                        Some(path) => response.on_hover_text(path.as_str()),
                        None => response,
                    };
                    if response.clicked() {
                        action(events, "file.open_recent", json!({ "index": i }));
                    }
                }
            }
            ui.add_space(12.0);
            let sample = ui.add_enabled(
                shell.has_sample(),
                egui::Button::new(strings::text("start-sample")),
            );
            if sample.clicked() {
                action(events, "file.open_sample", Value::Null);
            }
            sample.on_disabled_hover_text(strings::text("error-no-sample"));
            if ui.button(strings::text("start-import-qet")).clicked() {
                action(events, "file.import_qet", Value::Null);
            }
            if ui.button(strings::text("start-company-standard")).clicked() {
                action(events, "tools.company_standard", Value::Null);
            }
            if let Some(basic) = shell.settings().basic_project.clone()
                && shell.default_template_name().is_some()
                && ui.button(strings::text("start-edit-standard")).clicked()
            {
                action(events, "file.open", json!({ "dir": basic }));
            }
            if ui.button(strings::text("start-tutorials")).clicked() {
                action(events, "help.tutorials", Value::Null);
            }
            ui.add_space(16.0);
            let settings = shell.settings();
            quick_switch(
                ui,
                events,
                "start-quick-language",
                "language",
                &settings.language,
                &LANGUAGES,
            );
            quick_switch(
                ui,
                events,
                "start-quick-keymap",
                "keymap",
                &settings.keymap,
                &KEYMAPS,
            );
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.weak(if settings.update_check {
                    strings::text("start-update-check-on")
                } else {
                    strings::text("start-update-check")
                });
                if ui.small_button(strings::text("start-privacy-change")).clicked() {
                    action(events, "help.first_run", json!({ "step": "privacy" }));
                }
            });
        });
    });
}
