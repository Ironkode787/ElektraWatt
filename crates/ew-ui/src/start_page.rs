// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start page (WP-S2.01 part 1, spec section 5.3; UX/UI doc 16 section 3.7, part-1 subset): Open..., the recent
//! projects, and the line "Update check: off" (nothing on the page opens a connection, NFR-PRIV-01). It appears
//! before any project is touched; no work that could take time runs before the first frame.

use egui::Ui;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

/// Paints the start page into `ui`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.2);
        ui.heading(strings::text("start-title"));
        ui.add_space(16.0);
        if ui.button(strings::text("start-open")).clicked() {
            events.push(Event::Action("file.open", Value::Null));
        }
        let names = shell.recent_names();
        if !names.is_empty() {
            ui.add_space(16.0);
            ui.label(strings::text("start-recent"));
            for (i, name) in names.iter().enumerate() {
                let response = ui.button(name.as_str());
                let response = match shell.recent_paths().get(i) {
                    Some(path) => response.on_hover_text(path.as_str()),
                    None => response,
                };
                if response.clicked() {
                    events.push(Event::Action("file.open_recent", json!({ "index": i })));
                }
            }
        }
        ui.add_space(24.0);
        ui.weak(strings::text("start-update-check"));
    });
}
