// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock dialog's body (Project ▸ Edit stock…, doc 07 §2.5): the stock as a table in its order (kind,
//! part number, description, what it is for, colour) with Up, Down and Remove per row, a part number to add, the cut
//! increment, and the dialog's refusal under them. Apply, Add the generic stock and Cancel are the dialog's buttons.

use egui::{RichText, TextEdit, Ui, vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::stock::StockDialog;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::theme;

/// The body.
pub(crate) fn stock(ui: &mut Ui, d: &StockDialog, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let small = |text: &str| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    if d.rows.is_empty() {
        ui.label(small(strings::text(key!("stock-empty"))));
    } else {
        egui::ScrollArea::vertical()
            .id_salt("stock-rows")
            .max_height(t.size_row * 12.0)
            .show(ui, |ui| {
                egui::Grid::new("stock-table")
                    .num_columns(8)
                    .spacing(vec2(t.space_3, t.space_2))
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(small("#"));
                        ui.label(small(strings::text(key!("stock-column-kind"))));
                        ui.label(small(strings::text(key!("stock-column-number"))));
                        ui.label(small(strings::text(key!("stock-column-description"))));
                        ui.label(small(strings::text(key!("stock-column-for"))));
                        ui.label(small(strings::text(key!("stock-column-colour"))));
                        ui.label("");
                        ui.label("");
                        ui.end_row();
                        for (i, r) in d.rows.iter().enumerate() {
                            let n = i + 1;
                            ui.label(n.to_string());
                            ui.label(strings::choice_text(&r.kind));
                            ui.label(RichText::new(&r.number).font(theme::font(&t.type_body_strong)));
                            ui.label(&r.description);
                            let what = if r.marks.is_empty() {
                                if r.size.is_empty() {
                                    String::new()
                                } else {
                                    format!("{} mm\u{b2}", r.size)
                                }
                            } else {
                                strings::field_choice_text("part", "sections.marker.marks", &r.marks)
                            };
                            ui.label(what);
                            ui.label(&r.colour);
                            ui.horizontal(|ui| {
                                for (op, label) in [("up", key!("stock-up")), ("down", key!("stock-down"))] {
                                    if ui.small_button(strings::text(label)).clicked() {
                                        events.push(Event::Action(
                                            "workshop.stock_row",
                                            json!({ "row": n, "op": op }),
                                        ));
                                    }
                                }
                            });
                            if ui.small_button(strings::text(key!("stock-remove"))).clicked() {
                                events.push(Event::Action(
                                    "workshop.stock_row",
                                    json!({ "row": n, "op": "remove" }),
                                ));
                            }
                            ui.end_row();
                        }
                    });
            });
    }
    ui.add_space(t.space_3);
    // A part by its number: held, or from a library.
    let id = egui::Id::unique("stock-add-field");
    let mut text = ui.data(|m| m.get_temp::<String>(id)).unwrap_or_default();
    crate::components::form_row(ui, strings::text(key!("stock-add-field")), |ui, label| {
        let r = ui
            .add(TextEdit::singleline(&mut text).desired_width(t.size_control * 8.0))
            .labelled_by(label);
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button(strings::text(key!("stock-add"))).clicked() || enter) && !text.trim().is_empty() {
            events.push(Event::Action(
                "workshop.stock_add",
                json!({ "part": text.trim() }),
            ));
            text.clear();
        }
    });
    ui.data_mut(|m| m.insert_temp(id, text));
    let mut cut = d.cut.clone();
    crate::components::form_row(ui, strings::text(key!("stock-cut")), |ui, label| {
        let r = ui
            .add(TextEdit::singleline(&mut cut).desired_width(t.size_control * 4.0))
            .labelled_by(label);
        if r.changed() {
            events.push(Event::Action("workshop.stock_cut", json!({ "text": cut })));
        }
    });
    ui.label(small(strings::text(key!("stock-cut-hint"))));
    if let Some(e) = &d.error {
        ui.label(RichText::new(e).color(t.error.color32()));
    }
}
