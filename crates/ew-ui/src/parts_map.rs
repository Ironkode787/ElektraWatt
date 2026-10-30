// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The column mapping step of File, Import, Parts table… (FT-649, WP-S1.19 §9): a card floating at the top centre
//! while `ShellModel::parts_map` is open, before the import card (doc 16a §6.9, `elevation.1`).
//!
//! **Layout.** "Map columns" in `type.title`, one line naming the file, the Source field (the key the map is saved
//! under) with the "Saved map" caption when the members came from it, then the column table: Column, Sample (in
//! `color.text.secondary`) and "Imports as", a drop-down of the members in words ("Not imported" first, then
//! "Manufacturer", "Part number", …, "Technical data: `<header>`"). The importer's refusal of the map stands under the
//! table in words in `color.error` ("Map a column to Manufacturer"); Import (Primary, disabled with that reason as its tooltip) and
//! Cancel close the card.
//!
//! **Input.** The Source field dispatches `parts.map {source}` on Enter or when it loses the focus; a member choice
//! `parts.map {column, member}`; Import `parts.map_import`; Cancel and Escape `ui.dismiss`.

use egui::{Align, Align2, Id, Layout, RichText, Vec2};
use ew_viewmodel::shell::{PartsMap, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{Kind, Tip, button};

/// Paints the mapping card when the step is open.
pub fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(map) = shell.parts_map() else {
        return;
    };
    let t = crate::theme::current(ctx);
    egui::Area::new(Id::unique("parts-map-card"))
        .anchor(
            Align2::CENTER_TOP,
            [0.0, t.size_tab_row + t.size_command_row + t.space_4],
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(t.size_dialog_form.max(t.size_dialog_small));
            egui::Frame::new()
                .shadow(crate::theme::shadow(&t.elevation_1))
                .corner_radius(crate::theme::radius(t.radius_md))
                .show(ui, |ui| {
                    crate::components::card_frame(&t).show(ui, |ui| body(ui, map, events));
                });
        });
}

fn body(ui: &mut egui::Ui, map: &PartsMap, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    ui.spacing_mut().item_spacing.y = t.space_3;
    ui.label(
        RichText::new(strings::text("parts-map-title"))
            .font(crate::theme::font(&t.type_title))
            .color(t.text_primary.color32()),
    );
    ui.label(
        RichText::new(strings::format("parts-map-intro", &[&map.file]))
            .font(crate::theme::font(&t.type_body))
            .color(t.text_secondary.color32()),
    );
    source_row(ui, map, events);
    columns(ui, map, events);
    // Cycle 37 (window check W37): the refusal in words ("Map a column to Manufacturer").
    if let Some(e) = map.error_text() {
        crate::components::live_line(ui, &e, true);
    }
    buttons(ui, map, events);
}

/// The Source field and the "Saved map" caption.
fn source_row(ui: &mut egui::Ui, map: &PartsMap, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    let id = Id::unique("parts-map-source");
    ui.horizontal(|ui| {
        let label = ui
            .allocate_ui_with_layout(
                Vec2::new(t.size_dialog_label, t.size_control),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(t.size_dialog_label);
                    ui.label(
                        RichText::new(strings::text("parts-map-source"))
                            .font(crate::theme::font(&t.type_label))
                            .color(t.text_secondary.color32()),
                    )
                },
            )
            .inner;
        let focused = ui.memory(|m| m.has_focus(id.with("edit")));
        let mut text = if focused {
            ui.data(|d| d.get_temp::<String>(id))
                .unwrap_or_else(|| map.source.clone())
        } else {
            map.source.clone()
        };
        let r = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(id.with("edit"))
                .desired_width(t.size_dialog_label * 1.5),
        );
        r.clone().labelled_by(label.id);
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
        if r.lost_focus() && text.trim() != map.source {
            events.push(Event::Action("parts.map", json!({ "source": text.trim() })));
        }
        if map.saved {
            crate::components::caption_badge(ui, strings::text("parts-map-saved"));
        }
    });
}

/// The column table: header, sample and the member drop-down per column.
fn columns(ui: &mut egui::Ui, map: &PartsMap, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    let none = strings::text("parts-map-none");
    let small = |s: &str, strong: bool| {
        RichText::new(s.to_owned())
            .font(crate::theme::font(if strong {
                &t.type_label
            } else {
                &t.type_small
            }))
            .color(if strong {
                t.text_secondary.color32()
            } else {
                t.text_tertiary.color32()
            })
    };
    egui::ScrollArea::vertical()
        .id_salt("parts-map-columns")
        .max_height(t.size_control * 9.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("parts-map-grid")
                .num_columns(3)
                .spacing([t.space_4, t.space_2])
                .min_col_width(t.size_dialog_label * 0.6)
                .show(ui, |ui| {
                    ui.label(small(strings::text("parts-map-column"), true));
                    ui.label(small(strings::text("parts-map-sample"), true));
                    ui.label(small(strings::text("parts-map-member"), true));
                    ui.end_row();
                    for (i, c) in map.columns.iter().enumerate() {
                        ui.label(
                            RichText::new(&c.header)
                                .font(crate::theme::font(&t.type_body))
                                .color(t.text_primary.color32()),
                        );
                        ui.add(egui::Label::new(small(&c.sample, false)).truncate());
                        let mut choices = PartsMap::choices(&c.header);
                        if let Some(m) = &c.member
                            && !choices.contains(m)
                        {
                            choices.push(m.clone());
                        }
                        // Cycle 37 (window check W37): the members in words ("Part number"); the actions keep
                        // their names.
                        let shown = c
                            .member
                            .as_deref()
                            .map_or_else(|| none.to_owned(), |m| PartsMap::member_text(m, &c.header));
                        let combo = egui::ComboBox::from_id_salt(("parts-map-member", i))
                            .selected_text(shown)
                            .width(t.size_dialog_label * 1.4)
                            .icon(crate::components::combo_chevron)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(c.member.is_none(), none).clicked() {
                                    events.push(Event::Action(
                                        "parts.map",
                                        json!({"column": c.header, "member": ""}),
                                    ));
                                }
                                for m in &choices {
                                    if ui
                                        .selectable_label(
                                            c.member.as_deref() == Some(m.as_str()),
                                            PartsMap::member_text(m, &c.header),
                                        )
                                        .clicked()
                                    {
                                        events.push(Event::Action(
                                            "parts.map",
                                            json!({"column": c.header, "member": m}),
                                        ));
                                    }
                                }
                            });
                        // Named "Imports as <header>" (every focusable node has a name, ADR-0014 amendment 2).
                        let name = format!("{} {}", strings::text("parts-map-member"), c.header);
                        ui.ctx()
                            .accesskit_node_builder(combo.response.id, |n| n.set_label(name));
                        ui.end_row();
                    }
                });
        });
}

/// Import (Primary, disabled with the map's refusal as its tooltip) and Cancel, right-aligned.
fn buttons(ui: &mut egui::Ui, map: &PartsMap, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    let size = Vec2::new(ui.available_width(), t.size_control);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        let order: [bool; 2] = if cfg!(target_os = "windows") {
            [false, true]
        } else {
            [true, false]
        };
        for import in order {
            if import {
                let enabled = map.can_import();
                let r = button(ui, Kind::Primary, strings::text("parts-map-import"), enabled, 0.0);
                let r = match (map.error_text(), enabled) {
                    (Some(reason), false) => r.disabled_tip(&reason),
                    _ => r,
                };
                if r.clicked() {
                    events.push(Event::Action("parts.map_import", json!({})));
                }
            } else if button(ui, Kind::Secondary, strings::text("parts-map-cancel"), true, 0.0).clicked() {
                events.push(Event::Action("ui.dismiss", json!({})));
            }
        }
    });
}
