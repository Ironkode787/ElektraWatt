// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker popup (WP-S2.04 §7.1): what can be placed on the focused page, a filter field, one row per symbol
//! and per template function, disabled rows with their reasons. Up, Down, Home and End move the row (`picker.row`),
//! Enter chooses (`picker.choose`), typing filters (`picker.filter`), Escape closes it (`ui.dismiss`, the topmost
//! popup). Every change is a registry action, so the headless mode drives the same model.

use egui::{Align2, Button, Id, Key, Modifiers, ScrollArea, TextEdit, Window};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{PickerMode, PickerRow};
use serde_json::json;

use crate::app::Event;

/// Paints the picker when it is open. `just_opened` puts the keyboard focus into its filter field.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let Some(picker) = shell.picker() else {
        return;
    };
    // The list's keys are consumed before the filter field sees them.
    let (down, up, home, end, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Home),
            i.consume_key(Modifiers::NONE, Key::End),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    for (hit, params) in [
        (down, json!({"step": 1})),
        (up, json!({"step": -1})),
        (home, json!({"to": 0})),
        (end, json!({"to": -1})),
    ] {
        if hit {
            events.push(Event::Action("picker.row", params));
        }
    }
    if enter {
        events.push(Event::Action("picker.choose", json!({})));
    }
    let title = match picker.mode {
        PickerMode::Exchange => strings::text("picker-title-exchange"),
        PickerMode::Place | PickerMode::Parts => strings::text("picker-title"),
    };
    Window::new(title)
        .id(Id::unique("place-picker"))
        .anchor(Align2::CENTER_TOP, [0.0, 60.0])
        .collapsible(false)
        .resizable(false)
        .default_width(520.0)
        .show(ctx, |ui| {
            let mut filter = picker.filter.clone();
            let edit = ui.add(
                TextEdit::singleline(&mut filter)
                    .hint_text(strings::text("picker-filter-hint"))
                    .desired_width(f32::INFINITY),
            );
            if just_opened {
                edit.request_focus();
            }
            if filter != picker.filter {
                events.push(Event::Action("picker.filter", json!({ "text": filter })));
            }
            ui.separator();
            if picker.rows.is_empty() {
                ui.label(strings::text("picker-empty"));
            }
            ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for (i, row) in picker.rows.iter().enumerate() {
                    let text = match *row {
                        PickerRow::Symbol(k) => {
                            let s = &picker.catalogue.symbols[k];
                            let badge = if s.copy.is_some() {
                                strings::text("picker-badge-embedded")
                            } else {
                                strings::text("picker-badge-starter")
                            };
                            format!("{}  [{badge}]", s.key)
                        }
                        PickerRow::PartFn(p, k) => {
                            let part = &picker.catalogue.parts[p];
                            let f = &part.functions[k];
                            let mut text = format!(
                                "{} {}  {}",
                                part.part_number,
                                part.manufacturer,
                                strings::format(
                                    "picker-part-function",
                                    &[
                                        &(k + 1).to_string(),
                                        &part.functions.len().to_string(),
                                        &f.fndef_key
                                    ],
                                )
                            );
                            if f.main {
                                text.push(' ');
                                text.push_str(strings::text("picker-main"));
                            }
                            text
                        }
                    };
                    ui.horizontal(|ui| {
                        let reason = picker.disabled(*row);
                        let button = Button::new(text).selected(i == picker.row);
                        if ui.add_enabled(reason.is_none(), button).clicked() {
                            events.push(Event::Action("picker.row", json!({ "to": i })));
                            events.push(Event::Action("picker.choose", json!({})));
                        }
                        if let Some(reason) = reason {
                            ui.weak(strings::text(reason));
                        }
                    });
                }
            });
        });
}
