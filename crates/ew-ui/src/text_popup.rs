// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text popup (WP-S2.04 §6.4 item 1, §10.5): the Text tool's and text edit's single-line field. While the Text
//! tool types or Text edit is open, a window at the top holds one field; Enter sends `text.input` and `tool.finish`
//! (so the tool places or changes the text), Escape is the window's popup dismissal (the key routing sends
//! `ui.dismiss`, which reaches the tool). The buffer starts as the ghost's text and is dropped when the popup closes.

use egui::{Align2, Id, Window};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// Paints the popup when the active tool has one.
pub fn show(ctx: &egui::Context, shell: &ShellModel, buffer: &mut Option<String>, events: &mut Vec<Event>) {
    let Some(popup) = shell.text_popup() else {
        *buffer = None;
        return;
    };
    let first = buffer.is_none();
    if first {
        *buffer = Some(popup.text.clone());
    }
    let Some(text) = buffer.as_mut() else {
        return;
    };
    Window::new(strings::text(popup.title_key))
        .id(Id::unique("text-popup"))
        .anchor(Align2::CENTER_TOP, [0.0, 60.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(strings::text("action-text-input"));
            let edit = ui.add(egui::TextEdit::singleline(text).desired_width(320.0));
            // Named for assistive technology (and the tests), as the tool strip's fields are.
            let label = strings::text("action-text-input").to_owned();
            edit.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
            if first {
                edit.request_focus();
            }
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                events.push(Event::Action("text.input", json!({ "text": text.clone() })));
                events.push(Event::Action("tool.finish", json!({})));
            }
        });
}
