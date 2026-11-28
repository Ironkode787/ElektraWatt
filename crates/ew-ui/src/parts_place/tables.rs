// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A declared table of the sheet of Parts (doc 07 §5.4): Comes with, Also sold as, the doors, the holes; its rows as
//! cells by their value types, a row made in passing where the table names parts (New part… of Comes with).

use egui::{RichText, Ui, vec2};
use ew_viewmodel::shell::parts_place::table_add_word;
use ew_viewmodel::{key, strings};
use serde_json::json;

use super::{field_message, table_cell};
use crate::app::Event;
use crate::theme;

/// One declared table of the part: the item's members as columns (the key column first, read-only), a cell per
/// member by its value type, a remove button per row and the Add button where rows are added here, and the table's
/// first message under it.
pub(super) fn declared_table(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    table: &ew_viewmodel::ew_api::DraftTable,
    editable: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let header = |c: &str| ew_viewmodel::shell::part_dialog::field_label(&format!("{}[].{c}", table.path));
    let small_secondary = |text: String| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    if table.rows.is_empty() {
        ui.label(RichText::new(strings::text(key!("part-table-empty"))).color(t.text_secondary.color32()));
    } else {
        egui::Grid::new(("part-table", table.path))
            .num_columns(table.columns.len() + 3)
            .spacing(vec2(t.space_3, t.space_2))
            .striped(true)
            .show(ui, |ui| {
                if let Some(k) = table.key {
                    ui.label(small_secondary(header(k)));
                }
                for (c, _) in &table.columns {
                    ui.label(small_secondary(header(c)));
                }
                ui.end_row();
                for (i, cells) in table.rows.iter().enumerate() {
                    if table.key.is_some() {
                        let key_text = table.keys.get(i).map_or("", String::as_str);
                        ui.label(RichText::new(key_text).font(theme::font(&t.type_body_strong)));
                    }
                    for ((member, ty), text) in table.columns.iter().zip(cells) {
                        let field = format!("{}[{i}].{member}", table.path);
                        let name = format!("{} {}", header(member), i + 1);
                        if editable {
                            table_cell(ui, &field, &name, ty, (text, None), events);
                        } else {
                            // A choice reads in its words.
                            let words = match ty.inner() {
                                ew_viewmodel::ew_api::output::FieldType::Enum { .. } if !text.is_empty() => {
                                    strings::field_choice_text("part", &field, text)
                                }
                                _ => text.clone(),
                            };
                            ui.label(RichText::new(words).color(t.text_secondary.color32()));
                        }
                    }
                    // A part it comes with made in passing (doc 07 §5.5), as a cabinet's plate is: Save returns to
                    // this row with it named.
                    if editable && table.path == "accessories" {
                        let new = strings::text(key!("parts-new-accessory"));
                        if ui.small_button(new).clicked() {
                            events.push(Event::Action(
                                "parts.new",
                                json!({ "start": "accessory", "for_field": format!("accessories[{i}].part") }),
                            ));
                        }
                    }
                    if editable && table.key.is_none() {
                        let remove = strings::format(key!("part-table-remove"), &[&(i + 1).to_string()]);
                        if ui.small_button("\u{d7}").on_hover_text(&remove).clicked() {
                            events.push(Event::Action(
                                "parts.row",
                                json!({ "table": table.path, "remove": i + 1 }),
                            ));
                        }
                    }
                    ui.end_row();
                }
            });
    }
    let prefix = format!("{}[", table.path);
    if let Some(m) = d.messages.iter().find(|m| m.field.starts_with(&prefix))
        && let Some(text) = field_message(d, &m.field)
    {
        ui.label(RichText::new(text).color(t.error.color32()));
    }
    if editable
        && table.key.is_none()
        && let Some(add) = table_add_word(table.path)
    {
        ui.add_space(t.space_2);
        if ui.button(format!("+ {add}")).clicked() {
            events.push(Event::Action("parts.row", json!({ "table": table.path })));
        }
    }
}
