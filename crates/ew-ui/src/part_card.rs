// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part's data at a glance (F22): the card under the device browser's picture, on a device's row in the Devices
//! list (its tooltip) and behind Part details in Properties. Read only: the identity and technical data as rows, the
//! size with the part's outline drawn to scale beside a 35 mm rail, what it gives, where it comes from, and Edit part…
//! for a part the project holds.

use egui::{Color32, RichText, Stroke, Ui, Vec2};
use ew_viewmodel::key;
use ew_viewmodel::part_card::PartCardView;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::theme::{self, Tokens};

/// The height of the outline's drawing, in points.
const OUTLINE_H: f32 = 72.0;
/// The comparison rail's height, 35 mm, in nanometres.
const RAIL_NM: f32 = 35_000_000.0;

/// One line of `text` in `font` and `colour`, wrapped; nothing for an empty text.
fn line(ui: &mut Ui, text: &str, font: &theme::TypeToken, colour: theme::Rgba) {
    if !text.is_empty() {
        ui.add(
            egui::Label::new(
                RichText::new(text)
                    .font(theme::font(font))
                    .color(colour.color32()),
            )
            .wrap(),
        );
    }
}

/// The card's body: everything but the name, which the caller shows (`title` adds the manufacturer and number line).
pub fn body(ui: &mut Ui, t: &Tokens, view: &PartCardView, title: bool, events: &mut Vec<Event>) {
    let words = &view.words;
    if title {
        line(ui, &words.title, &t.type_body_strong, t.text_primary);
        line(ui, &words.description, &t.type_small, t.text_secondary);
    }
    // The rows: the label in a column of 45 % of the card, the value beside it.
    let label_w = ui.available_width() * 0.45;
    for (label, value) in &words.rows {
        ui.horizontal_top(|ui| {
            ui.allocate_ui(Vec2::new(label_w, 0.0), |ui| {
                ui.set_width(label_w);
                line(ui, label, &t.type_small, t.text_tertiary);
            });
            line(ui, value, &t.type_small, t.text_secondary);
        });
    }
    line(
        ui,
        strings::text(key!("card-size")),
        &t.type_small,
        t.text_tertiary,
    );
    match &view.card.size {
        Some(size) if size.width.is_some() && size.height.is_some() => {
            line(ui, &words.size, &t.type_small, t.text_secondary);
            outline(ui, t, size.width.unwrap_or(0), size.height.unwrap_or(0));
        }
        _ => line(
            ui,
            strings::text(key!("card-no-size")),
            &t.type_small,
            t.text_secondary,
        ),
    }
    if !words.gives.is_empty() {
        line(
            ui,
            strings::text(key!("card-gives")),
            &t.type_small,
            t.text_tertiary,
        );
        line(ui, &words.gives.join(" \u{b7} "), &t.type_small, t.text_secondary);
    }
    line(
        ui,
        strings::text(key!("card-from")),
        &t.type_small,
        t.text_tertiary,
    );
    line(ui, &words.source, &t.type_small, t.text_secondary);
    if view.card.held {
        let edit = strings::text(key!("action-part-edit-card"));
        if components::button(ui, Kind::Secondary, edit, true, 0.0).clicked() {
            let part = format!("id:{}", view.card.part);
            events.push(Event::Action("part.edit", json!({ "part": part })));
        }
    }
}

/// The part's front, `width` × `height` nanometres, drawn to scale over a 35 mm rail band, as it sits on a rail; the
/// scale fits the taller of the two into the drawing's height.
fn outline(ui: &mut Ui, t: &Tokens, width: i64, height: i64) {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let avail = ui.available_width();
    let scale = (OUTLINE_H / h.max(RAIL_NM)).min(avail * 0.8 / w);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(avail, OUTLINE_H), egui::Sense::hover());
    let label = strings::text(key!("card-size-beside-rail"));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label));
    let painter = ui.painter_at(rect);
    let centre = rect.center();
    let rail = egui::Rect::from_center_size(centre, Vec2::new(avail, RAIL_NM * scale));
    painter.rect_filled(rail, 0.0, t.surface_sunken.color32());
    let part = egui::Rect::from_center_size(centre, Vec2::new(w * scale, h * scale));
    painter.rect_filled(part, 0.0, Color32::WHITE);
    painter.rect_stroke(
        part,
        0.0,
        Stroke::new(1.0, t.text_secondary.color32()),
        egui::StrokeKind::Inside,
    );
    response.on_hover_text(label);
}
