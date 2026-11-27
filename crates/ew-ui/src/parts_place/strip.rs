// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The strips over Parts' drawing (doc 07 §5.6): the sentence of what is drawn and for what, with its way back, and
//! on a piece's drawing Use it for every part with this piece… with the card that names the parts it writes.

use egui::{RichText, Ui, UiBuilder};
use ew_viewmodel::{key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::theme;

/// The strip over the drawing (doc 07 §5.6): what is drawn and for what, and the way back; the bottom of what it took
/// of `rect` (its top where the drawing has no strip).
pub(crate) fn drawing_strip(
    ui: &mut Ui,
    rect: egui::Rect,
    editor: &ew_viewmodel::shell::symbol_editor::SymbolEditor,
    events: &mut Vec<Event>,
) -> f32 {
    let Some(sentence) = editor.strip() else {
        return rect.top();
    };
    let piece = matches!(
        editor.purpose,
        ew_viewmodel::shell::symbol_editor::Drawing::Piece(_)
    ) && editor.device_box().is_none();
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        if piece {
            // A piece's symbol: the way to make it the symbol of that kind of piece for his parts (the way back is
            // the bar's To the part).
            let link = strings::text(key!("parts-use-for-all"));
            strip_with(ui, &sentence, link, ("parts.use_for_all", json!({})), events);
            if let Some(parts) = &editor.use_for_all {
                use_for_all_card(ui, parts, events);
            }
        } else {
            strip(ui, &sentence, editor.back_label(), events);
        }
    })
    .response
    .rect
    .bottom()
}

/// The card of Use it for every part with this piece… (doc 07 §5.6): the parts it would change, by number, and how
/// many the project holds, before Apply; Cancel closes it.
pub(super) fn use_for_all_card(
    ui: &mut Ui,
    parts: &[ew_viewmodel::ew_api::piece_symbols::DrawnPart],
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    egui::Frame::NONE
        .fill(t.surface_raised.color32())
        .stroke(theme::stroke(t.border_width, t.border_default))
        .inner_margin(egui::Margin::symmetric(t.space_4 as i8, t.space_2 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if parts.is_empty() {
                ui.label(strings::text(key!("parts-use-for-all-none")));
            } else {
                let names: Vec<&str> = parts.iter().map(|p| p.number.as_str()).collect();
                ui.add(
                    egui::Label::new(strings::format(
                        key!("parts-use-for-all-card"),
                        &[&names.join(", ")],
                    ))
                    .wrap(),
                );
                let held = parts.iter().filter(|p| p.held).count();
                if held > 0 {
                    ui.label(strings::format(
                        key!("parts-use-for-all-held"),
                        &[&held.to_string()],
                    ));
                }
            }
            ui.horizontal(|ui| {
                if !parts.is_empty()
                    && components::button(
                        ui,
                        components::Kind::Primary,
                        strings::text(key!("parts-apply")),
                        None,
                        0.0,
                    )
                    .clicked()
                {
                    events.push(Event::Action("parts.use_for_all", json!({ "apply": true })));
                }
                if components::button(
                    ui,
                    components::Kind::Secondary,
                    strings::text(key!("parts-cancel")),
                    None,
                    0.0,
                )
                .clicked()
                {
                    events.push(Event::Action("parts.use_for_all", json!({ "cancel": true })));
                }
            });
        });
}

/// The strip over the sheet (doc 16a §6.27): where the person came from, and the way back.
pub(super) fn strip(ui: &mut Ui, sentence: &str, back: &str, events: &mut Vec<Event>) {
    strip_with(ui, sentence, back, ("parts.back", json!({})), events);
}

/// A strip with its sentence and one button at its right, which runs `action`.
pub(super) fn strip_with(
    ui: &mut Ui,
    sentence: &str,
    button: &str,
    action: (&'static str, serde_json::Value),
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    egui::Frame::NONE
        .fill(t.accent_subtle.color32())
        .inner_margin(egui::Margin::symmetric(t.space_4 as i8, t.space_2 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The sentence wraps in what the button leaves; it never runs under it.
            let font = theme::font(&t.type_body);
            let reserve = ui
                .painter()
                .layout_no_wrap(button.to_owned(), font.clone(), theme::NONE)
                .size()
                .x
                + 4.0 * t.space_4;
            ui.horizontal(|ui| {
                let room = egui::vec2((ui.available_width() - reserve).max(0.0), 0.0);
                ui.allocate_ui(room, |ui| {
                    ui.add(egui::Label::new(RichText::new(sentence).font(font)).wrap());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if components::button(ui, components::Kind::Secondary, button, None, 0.0).clicked() {
                        events.push(Event::Action(action.0, action.1.clone()));
                    }
                });
            });
        });
}
