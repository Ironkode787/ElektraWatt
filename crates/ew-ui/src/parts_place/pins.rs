// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sheet's tables of a part's pins (doc 07 §5.4): a terminal's levels with their clamps (picture v4), and
//! Connections, the pins' clamps with the pins of the same clamp on one row.

use egui::{RichText, Sense, Ui, vec2};
use ew_viewmodel::{key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::components::Tip as _;
use crate::icons::IconId;
use crate::theme;

/// On the schematic of a terminal (doc 07 §5.4, picture v4): how many levels, then a row per level from the top, its
/// left and right clamp and that the two are joined inside. The levels make the terminal's pins, each at its level.
pub(super) fn levels_section(ui: &mut Ui, d: &ew_viewmodel::ew_api::PartSheet, editable: bool, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::part_pieces::MOST_LEVELS;
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        crate::parts_sheet::label(ui, strings::text(key!("parts-levels")), None);
        ui.add_space(t.space_3);
        ui.spacing_mut().item_spacing.x = t.space_1;
        for n in 1..=MOST_LEVELS {
            let r = ui
                .add_enabled(
                    editable,
                    egui::Button::selectable(d.levels.len() == n, n.to_string()),
                )
                .disabled_tip(strings::text(key!("reason-part-not-yours")));
            let name = strings::format(key!("parts-levels-n"), &[&n.to_string()]);
            r.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Button, editable, d.levels.len() == n, &name)
            });
            if r.clicked() {
                events.push(Event::Action(
                    "parts.field",
                    json!({ "field": "levels", "value": n.to_string() }),
                ));
            }
        }
    });
    ui.add_space(t.space_2);
    egui::Grid::new("part-levels")
        .num_columns(4)
        .spacing(vec2(t.space_5, t.space_2))
        .show(ui, |ui| {
            super::table_header(ui, strings::text(key!("parts-level")));
            super::table_header(ui, strings::text(key!("parts-level-left")));
            super::table_header(ui, strings::text(key!("parts-level-right")));
            super::table_header(ui, strings::text(key!("parts-level-joined")));
            ui.end_row();
            // The upper level first, as the terminal stands.
            for (i, row) in d.levels.iter().enumerate().rev() {
                let n = (i + 1).to_string();
                ui.label(&n);
                for (member, value, key) in [
                    ("left", &row.left, key!("parts-level-left-of")),
                    ("right", &row.right, key!("parts-level-right-of")),
                ] {
                    if editable {
                        let name = strings::format(key, &[&n]);
                        if let Some(text) = super::table_text(ui, value, &name, t.size_control * 2.5) {
                            events.push(Event::Action(
                                "parts.field",
                                json!({ "field": format!("levels[{i}].{member}"), "value": text }),
                            ));
                        }
                    } else {
                        ui.label(value);
                    }
                }
                ui.label(
                    RichText::new(format!("{} \u{2013} {}", row.left, row.right))
                        .color(t.text_secondary.color32()),
                );
                ui.end_row();
            }
        });
    ui.add_space(t.space_1);
    ui.add(
        egui::Label::new(components::small_secondary(
            &t,
            strings::text(key!("parts-levels-note")),
        ))
        .wrap(),
    );
}

/// Connections (doc 07 §5.4, picture A): the pins' clamps, pins with the same clamp on one row (`1, 3, 5`); a value
/// typed on such a row is every one of its pins'; a row opened shows its pins one by one, each changed on its own.
pub(super) fn connections_table(
    ui: &mut Ui,
    d: &ew_viewmodel::ew_api::PartSheet,
    table: &ew_viewmodel::ew_api::DraftTable,
    editable: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let header = |c: &str| ew_viewmodel::shell::part_dialog::field_label(&format!("{}[].{c}", table.path));
    if table.rows.is_empty() {
        ui.label(RichText::new(strings::text(key!("part-table-empty"))).color(t.text_secondary.color32()));
        return;
    }
    // The rows of the same clamp, in the pins' order.
    let mut groups: Vec<(Vec<usize>, &Vec<String>)> = Vec::new();
    for (i, cells) in table.rows.iter().enumerate() {
        match groups.iter_mut().find(|(_, c)| *c == cells) {
            Some((rows, _)) => rows.push(i),
            None => groups.push((vec![i], cells)),
        }
    }
    let key_text = |i: usize| table.keys.get(i).map_or("", String::as_str);
    let cells_of = |ui: &mut Ui, rows: &[usize], cells: &Vec<String>, events: &mut Vec<Event>| {
        for ((member, ty), text) in table.columns.iter().zip(cells) {
            let first = rows[0];
            let field = format!("{}[{first}].{member}", table.path);
            let pins: Vec<&str> = rows.iter().map(|i| key_text(*i)).collect();
            let name = format!("{} {}", header(member), pins.join(", "));
            if !editable {
                let words = match ty.inner() {
                    ew_viewmodel::ew_api::output::FieldType::Enum { .. } if !text.is_empty() => {
                        strings::field_choice_text("part", &field, text)
                    }
                    _ => text.clone(),
                };
                ui.label(RichText::new(words).color(t.text_secondary.color32()));
                continue;
            }
            // A value typed on the row is every one of its pins'.
            let mut typed = Vec::new();
            let drawn = (*member == "face")
                .then(|| d.drawn_faces.get(first).map(String::as_str))
                .flatten();
            super::table_cell(ui, &field, &name, ty, (text, drawn), &mut typed);
            for e in typed {
                if let Event::Action(_, v) = &e {
                    let value = v["value"].clone();
                    for i in rows {
                        events.push(Event::Action(
                            "parts.field",
                            json!({ "field": format!("{}[{i}].{member}", table.path), "value": value }),
                        ));
                    }
                }
            }
        }
    };
    egui::Grid::new(("part-connections", table.path))
        .num_columns(table.columns.len() + 2)
        .spacing(vec2(t.space_3, t.space_2))
        .show(ui, |ui| {
            ui.label("");
            super::table_header(ui, strings::text(key!("parts-connections-pins")));
            for (c, _) in &table.columns {
                super::table_header(ui, &header(c));
            }
            ui.end_row();
            for (rows, cells) in &groups {
                let id = ui.scope_id().with(("connections-open", rows[0]));
                let open = ui.data(|m| m.get_temp::<bool>(id)).unwrap_or(false);
                let pins: Vec<&str> = rows.iter().map(|i| key_text(*i)).collect();
                if rows.len() > 1 {
                    let words = strings::format(
                        if open {
                            key!("parts-connections-close")
                        } else {
                            key!("parts-connections-open")
                        },
                        &[&pins.join(", ")],
                    );
                    let icon = if open {
                        IconId::chevron_down
                    } else {
                        IconId::chevron_right
                    };
                    let (rect, r) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::click());
                    crate::icons::paint(
                        ui.painter(),
                        icon,
                        rect,
                        t.text_secondary.color32(),
                        t.icon_stroke,
                    );
                    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &words));
                    if r.clicked() {
                        ui.data_mut(|m| m.insert_temp(id, !open));
                    }
                } else {
                    ui.label("");
                }
                ui.label(RichText::new(pins.join(", ")).font(theme::font(&t.type_body_strong)));
                cells_of(ui, rows, cells, events);
                ui.end_row();
                if open {
                    for i in rows {
                        ui.label("");
                        ui.label(key_text(*i));
                        cells_of(ui, &[*i], &table.rows[*i], events);
                        ui.end_row();
                    }
                }
            }
        });
    let prefix = format!("{}[", table.path);
    if let Some(m) = d.messages.iter().find(|m| m.field.starts_with(&prefix))
        && let Some(text) = super::field_message(d, &m.field)
    {
        ui.label(RichText::new(text).color(t.error.color32()));
    }
}
