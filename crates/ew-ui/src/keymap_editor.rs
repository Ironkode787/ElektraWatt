// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The keymap editor (WP-S2.08 part 2, D8; doc 16 §4.4): Help ▸ Keyboard shortcuts made editable, inside the form
//! dialog of doc 16a §6.12 (its width grows with the window, `dialog::form_width`). A search field over the actions'
//! labels, IDs and keys; one row per action with its label, its key chips, where the keys apply (and what they shadow,
//! D8's info) and Record and Reset; a refused edit's errors in `color.error` above the list; the user keymap's unused
//! bindings; the note that the Menu key cannot be bound (FT-428). Import… and Export… are the dialog's footer buttons.
//!
//! Every control dispatches a `keymap.*` action; Record makes the shell record, and the next key press (not Escape,
//! which ends the recording through `ui.dismiss`) is sent as `keymap.record` with the chord's text.

use egui::{Align, Layout, RichText, Sense, Ui, vec2};
use ew_viewmodel::actions::Context;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::theme;

/// The rows the list shows before it scrolls.
const VISIBLE_ROWS: f32 = 12.0;

/// The width of a row's Record and Reset buttons (a dialog button's minimum, doc 16a §6.2).
const BUTTON_WIDTH: f32 = 72.0;

/// The label key of where a binding applies.
fn context_key(context: Context) -> &'static str {
    match context {
        Context::Global => "keymap-context-global",
        Context::Canvas => "keymap-context-canvas",
        Context::Navigator => "keymap-context-navigator",
        Context::Properties => "keymap-context-properties",
        Context::Find => "keymap-context-find",
    }
}

/// The recording: the first key press of this frame (Escape excepted) as `keymap.record`, taken out of the input so
/// that no widget or shortcut sees it.
fn record(ui: &Ui, action: &'static str, events: &mut Vec<Event>) {
    let keys = ui.ctx().input_mut(|i| {
        let mut found = None;
        i.events.retain(|e| match e {
            egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } if found.is_none() && *key != egui::Key::Escape => {
                match crate::keys::chord_text(*key, *physical_key, *modifiers) {
                    Some(text) => {
                        found = Some(text);
                        false
                    }
                    None => true,
                }
            }
            _ => true,
        });
        found
    });
    if let Some(keys) = keys {
        events.push(Event::Action(
            "keymap.record",
            json!({ "action": action, "keys": keys }),
        ));
    }
}

/// Paints the editor's content between the dialog's title and its footer.
pub(crate) fn show(ui: &mut Ui, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let state = shell.keymap_editor();
    if let Some(action) = state.recording {
        record(ui, action, events);
    }
    // The preset, and whether the user's keymap changes it.
    let preset = shell.keymap_preset().name();
    // W40a 5b: "with your changes" only while an action's bindings differ from the preset's.
    let preset_line = if ew_viewmodel::keymap::has_changes(shell.keymap_preset(), shell.user_keymap()) {
        strings::format("keymap-preset-custom", &[preset])
    } else {
        strings::format("keymap-preset", &[preset])
    };
    ui.label(
        RichText::new(preset_line)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32()),
    );
    let mut text = state.search.clone();
    let search = components::filter_field(ui, &mut text, strings::text("keymap-search-hint"));
    if just_opened {
        search.request_focus();
    }
    if search.changed() {
        events.push(Event::Action("keymap.search", json!({ "text": text })));
    }
    // A refused edit: what was tried and each error (D8).
    if let Some(refusal) = &state.refusal {
        let label = ew_viewmodel::actions::find(refusal.action)
            .map_or(refusal.action, |m| strings::text(m.label_key));
        let what = if refusal.chord.is_empty() {
            label.to_owned()
        } else {
            format!("{} ({label})", refusal.chord)
        };
        components::live_line(ui, &strings::format("keymap-refused", &[&what]), true);
        for e in &refusal.errors {
            components::live_line(ui, e, true);
        }
    }
    let rows = shell.keymap_rows();
    let row_height = t.size_row;
    let width = ui.available_width();
    // Column widths as shares of the dialog's body, so they grow with the dialog (doc 16a §2, ID-908).
    // Five cells (three columns and two 72-px buttons) with four `space.3` gaps between them.
    let rest = (width - 2.0 * BUTTON_WIDTH - 4.0 * t.space_3).max(0.0);
    let (label_w, keys_w, where_w) = (rest * 0.40, rest * 0.25, rest * 0.35);
    if rows.is_empty() {
        ui.label(
            RichText::new(strings::text("keymap-none"))
                .font(theme::font(&t.type_body))
                .color(t.text_secondary.color32()),
        );
    }
    egui::ScrollArea::vertical()
        .max_height(row_height * VISIBLE_ROWS)
        .auto_shrink([false, true])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                ui.allocate_ui_with_layout(
                    vec2(width, row_height),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = t.space_3;
                        cell(ui, label_w, row_height, |ui| {
                            let mut text = RichText::new(&row.label)
                                .font(theme::font(&t.type_body))
                                .color(t.text_primary.color32());
                            if row.custom {
                                text = text.strong();
                            }
                            ui.add(egui::Label::new(text).truncate());
                        });
                        cell(ui, keys_w, row_height, |ui| {
                            if state.recording == Some(row.action) {
                                ui.label(
                                    RichText::new(strings::text("keymap-recording"))
                                        .font(theme::font(&t.type_small))
                                        .color(t.accent.color32()),
                                );
                            } else {
                                ui.spacing_mut().item_spacing.x = t.space_1;
                                for c in &row.chords {
                                    components::key_chip(ui, c);
                                }
                            }
                        });
                        cell(ui, where_w, row_height, |ui| {
                            let mut place = strings::text(context_key(row.context)).to_owned();
                            if row.custom {
                                place = format!("{place} · {}", strings::text("keymap-custom"));
                            }
                            // A short tag in the cell; the full text on hover and, for a changed row, below the list.
                            let (colour, text) = if row.notes.is_empty() {
                                (t.text_secondary, place)
                            } else {
                                (t.info, format!("{place} · {}", strings::text("keymap-shadows")))
                            };
                            let label = ui.add(
                                egui::Label::new(
                                    RichText::new(&text)
                                        .font(theme::font(&t.type_small))
                                        .color(colour.color32()),
                                )
                                .truncate()
                                .sense(Sense::hover()),
                            );
                            if !row.notes.is_empty() {
                                label.on_hover_text(row.notes.join("\n"));
                            }
                        });
                        if components::button(
                            ui,
                            Kind::Subtle,
                            strings::text("keymap-record"),
                            true,
                            BUTTON_WIDTH,
                        )
                        .clicked()
                        {
                            events.push(Event::Action(
                                "keymap.record_start",
                                json!({ "action": row.action }),
                            ));
                        }
                        if components::button(
                            ui,
                            Kind::Subtle,
                            strings::text("keymap-reset"),
                            row.custom,
                            BUTTON_WIDTH,
                        )
                        .clicked()
                        {
                            events.push(Event::Action("keymap.reset", json!({ "action": row.action })));
                        }
                    },
                );
            }
        });
    // W40a 5a: the Menu key's note only while a shortcut is being recorded, under the list that shows the recording.
    if state.recording.is_some() {
        ui.label(
            RichText::new(strings::text("keymap-menu-key"))
                .font(theme::font(&t.type_small))
                .color(t.text_tertiary.color32()),
        );
    }
    // What the user's own bindings shadow (D8's info), in full: the row's cell has room for a tag only.
    for note in rows.iter().filter(|r| r.custom).flat_map(|r| &r.notes) {
        ui.label(
            RichText::new(note)
                .font(theme::font(&t.type_small))
                .color(t.info.color32()),
        );
    }
    // The user keymap's bindings for actions this build does not know (D6).
    if let Some(user) = shell.user_keymap() {
        for b in user.unused() {
            let keys: Vec<String> = b.chords().map(ew_viewmodel::keymap::Chord::text).collect();
            ui.label(
                RichText::new(strings::format("keymap-unused", &[&b.action, &keys.join(", ")]))
                    .font(theme::font(&t.type_small))
                    .color(t.text_tertiary.color32()),
            );
        }
    }
}

/// One cell of a row: `width` wide, `height` high, its content left-aligned and cut at the right.
fn cell(ui: &mut Ui, width: f32, height: f32, content: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(width, height), Layout::left_to_right(Align::Center), |ui| {
        ui.set_width(width);
        ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
        content(ui);
    });
}
