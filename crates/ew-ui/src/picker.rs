// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker popup (WP-S2.04 §7.1): what can be placed on the focused page, a filter field, one row per symbol
//! and per template function (or, choosing a terminal, one per strip: FT-421), disabled rows with their reasons. Up, Down, Home and End move the row (`picker.row`),
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
        PickerMode::Strip => strings::text("picker-title-strip"),
        PickerMode::BlackBox => strings::text("picker-title-black-box"),
    };
    let t = crate::theme::current(ctx);
    Window::new(title)
        .id(Id::unique("place-picker"))
        .anchor(
            Align2::CENTER_TOP,
            [0.0, t.size_tab_row + t.size_command_row + t.space_4],
        )
        .collapsible(false)
        .resizable(false)
        .default_width(t.size_palette)
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
            // FT-605: a terminal chosen in a project without strips opens the strip chooser on an empty state that
            // says why and offers New terminal strip (doc 16a §6.17); `strip.new` lists the new strip as the current
            // row.
            if picker.mode == PickerMode::Strip && picker.empty_text() == Some("picker-empty-strips") {
                let new = crate::components::empty_state(
                    ui,
                    crate::icons::IconId::ew_terminal_strip,
                    strings::text("picker-empty-strips"),
                    strings::text("picker-empty-strips-text"),
                    Some(strings::text("picker-new-strip")),
                );
                if new.is_some_and(|r| r.clicked()) {
                    events.push(Event::Action("strip.new", json!({})));
                }
            } else if let Some(empty) = picker.empty_text() {
                ui.add_space(t.space_3);
                ui.label(egui::RichText::new(strings::text(empty)).color(t.text_secondary.color32()));
            }
            ScrollArea::vertical()
                .max_height(t.size_row * 13.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, row) in picker.rows.iter().enumerate() {
                        // FT-425: the manufacturer's header above its first part row.
                        if let Some(m) = picker.header_before(i) {
                            ui.add_space(t.space_2);
                            ui.label(
                                egui::RichText::new(m)
                                    .font(crate::theme::font(&crate::theme::TypeToken {
                                        semibold: true,
                                        ..t.type_small
                                    }))
                                    .color(t.text_secondary.color32()),
                            );
                        }
                        let (name, meta, tooltip) = row_texts(picker, *row);
                        let reason = picker.disabled(*row);
                        let meta = match reason {
                            Some(r) => strings::text(r).to_owned(),
                            None => meta,
                        };
                        let enabled = reason.is_none();
                        let text_colour = if enabled { t.text_primary } else { t.text_disabled };
                        // Name in body text, the library or function in tertiary (secondary on the selected row).
                        let meta_colour = if i == picker.row {
                            t.text_secondary
                        } else {
                            t.text_tertiary
                        };
                        let button = Button::new((
                            egui::RichText::new(&name).color(text_colour.color32()),
                            egui::Atom::grow(),
                            egui::RichText::new(&meta)
                                .font(crate::theme::font(&t.type_small))
                                .color(meta_colour.color32()),
                        ))
                        .selected(i == picker.row)
                        .frame_when_inactive(false)
                        .min_size(egui::vec2(ui.available_width(), t.size_row));
                        let response = ui.add_enabled(enabled, button).on_hover_text(&tooltip);
                        if response.clicked() {
                            events.push(Event::Action("picker.row", json!({ "to": i })));
                            events.push(Event::Action("picker.choose", json!({})));
                        }
                    }
                });
        });
}

/// A row's name, its library or function line, and its tooltip with the raw IDs (WP-S2.17 part 4, §8 item 6).
fn row_texts(picker: &ew_viewmodel::tools::Picker, row: PickerRow) -> (String, String, String) {
    match row {
        PickerRow::Symbol(k) => {
            let s = &picker.catalogue.symbols[k];
            let library = if s.copy.is_some() {
                strings::text("picker-badge-embedded")
            } else {
                strings::text("picker-badge-starter")
            };
            // The name is the key's last segment without its grid variant (`ew.iec.mcb-1p@qet10` → `mcb-1p`).
            let base = s.key.split('@').next().unwrap_or(&s.key);
            let name = base.rsplit('.').next().unwrap_or(base).to_owned();
            (name, library.to_owned(), format!("{}\n{}", s.key, s.fndef_key))
        }
        PickerRow::PartFn(p, k) => {
            let part = &picker.catalogue.parts[p];
            let f = &part.functions[k];
            let mut meta = strings::format(
                "picker-part-function",
                &[
                    &(k + 1).to_string(),
                    &part.functions.len().to_string(),
                    &f.fndef_key,
                ],
            );
            if f.main {
                meta.push(' ');
                meta.push_str(strings::text("picker-main"));
            }
            (
                part.part_number.clone(),
                meta,
                format!("{} {}\n{}", part.manufacturer, part.part_number, f.fndef_key),
            )
        }
        PickerRow::Strip(k) => {
            let st = &picker.catalogue.strips[k];
            (
                st.tag.clone(),
                strings::format("picker-strip-next", &[&st.next_designation]),
                st.tag.clone(),
            )
        }
    }
}
