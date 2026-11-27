// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The column mapping of an EPLAN report spreadsheet: a card floating in the window's centre, where a dialog stands
//! (at the top of the window it read as a strip left behind), while `ShellModel::parts_map` is open (doc 16a §6.9,
//! `elevation.1`). Its column table is the one Parts shows as the first step of a parts table's set ([`columns`]).
//!
//! **Layout.** "Map columns" in `type.title`, one line naming the file, the report's kind, the Source field (the key
//! the map is saved under) with the "Saved map" caption when the members came from it, then the column table: Column,
//! Sample (in `color.text.secondary`) and "Imports as", a drop-down of the members in words ("Not imported" first).
//! The refusal of the map stands under the table in words in `color.error`; Import (Primary, disabled with that reason
//! as its tooltip) and Cancel close the card.
//!
//! **Input.** The Source field dispatches `parts.map {source}` on Enter or when it loses the focus; a member choice
//! `parts.map {column, member}`; Import `parts.map_import`; Cancel and Escape `ui.dismiss`.

use egui::{Align, Align2, Id, Layout, RichText, Vec2};
use ew_viewmodel::ew_api::eplan::ReportTarget;
use ew_viewmodel::key;
use ew_viewmodel::shell::{MapKind, PartsMap, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{Kind, button};

/// Paints the mapping card when the step is open.
pub fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(map) = shell.parts_map() else {
        return;
    };
    let t = crate::theme::current(ctx);
    egui::Area::new(Id::unique("parts-map-card"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            // A form that grows with the window when its samples need the room: as wide as the column
            // table, from `size.dialog.form` up to the grown form width (`dialog::form_width`); nothing is stretched.
            let (names, samples, combo) = column_widths(ui, map);
            let need = names
                + samples
                + combo
                + 2.0 * t.space_4
                + ui.spacing().scroll.bar_width
                + 2.0 * (t.space_4 + t.border_width);
            let grown = crate::dialog::form_width(&t, ctx.content_rect().width());
            ui.set_width(need.max(t.size_dialog_form.min(grown)).min(grown));
            crate::components::lifted(&t).show(ui, |ui| {
                crate::components::card_frame(&t).show(ui, |ui| body(ui, map, events));
            });
        });
}

fn body(ui: &mut egui::Ui, map: &PartsMap, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    ui.spacing_mut().item_spacing.y = t.space_3;
    ui.label(crate::components::title(
        &t,
        strings::text(key!("parts-map-title")),
    ));
    ui.label(crate::components::body_secondary(
        &t,
        strings::format(key!("eplan-map-intro"), &[&map.file]),
    ));
    if let MapKind::Report(target) = map.kind {
        report_row(ui, target, events);
    }
    source_row(ui, map, events);
    columns(ui, map, 9.0, events);
    // The refusal in words ("Map a column to Manufacturer").
    if let Some(e) = map.error_text() {
        crate::components::live_line(ui, &e, true);
    }
    buttons(ui, map, events);
}

/// The kind of the EPLAN report the columns belong to: device list, terminal diagram or connection list
/// (`parts.map {target}`).
fn report_row(ui: &mut egui::Ui, target: ReportTarget, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    let words = |x: ReportTarget| {
        strings::text(match x {
            ReportTarget::Devices => key!("eplan-target-devices"),
            ReportTarget::Terminals => key!("eplan-target-terminals"),
            ReportTarget::Connections => key!("eplan-target-connections"),
        })
    };
    ui.horizontal(|ui| {
        let label = ui
            .allocate_ui_with_layout(
                Vec2::new(t.size_dialog_label, t.size_control),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(t.size_dialog_label);
                    ui.label(crate::components::styled(
                        strings::text(key!("eplan-map-list")),
                        &t.type_label,
                        t.text_secondary,
                    ))
                },
            )
            .inner;
        let combo = crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt("parts-map-report")
                .selected_text(words(target))
                .width(t.size_dialog_label * 1.5),
            |ui| {
                for (x, id) in [
                    (ReportTarget::Terminals, "terminals"),
                    (ReportTarget::Devices, "devices"),
                    (ReportTarget::Connections, "connections"),
                ] {
                    if ui.selectable_label(x == target, words(x)).clicked() {
                        events.push(Event::Action("parts.map", json!({ "target": id })));
                    }
                }
            },
        );
        combo.response.clone().labelled_by(label.id);
    });
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
                    ui.label(crate::components::styled(
                        strings::text(key!("parts-map-source")),
                        &t.type_label,
                        t.text_secondary,
                    ))
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
            crate::components::caption_badge(ui, strings::text(key!("parts-map-saved")));
        }
    });
}

/// The widths of the column table's columns: the widest Column name (`type.body`), the widest sample
/// (`type.small`), each at least `size.dialog.label` × 0.6, and the drop-downs' (`size.dialog.label` × 1.4).
fn column_widths(ui: &egui::Ui, map: &PartsMap) -> (f32, f32, f32) {
    let t = crate::components::tokens(ui);
    let narrow = t.size_dialog_label * 0.6;
    let widest = |font: egui::FontId, text: &dyn Fn(usize) -> String| {
        (0..map.columns.len())
            .map(|i| {
                ui.painter()
                    .layout_no_wrap(text(i), font.clone(), t.text_primary.color32())
                    .size()
                    .x
            })
            .fold(narrow, f32::max)
    };
    // The Column name column is as wide as its widest name or its heading ("Column in the file").
    let heading = ui
        .painter()
        .layout_no_wrap(
            strings::text(key!("parts-map-column")).to_owned(),
            crate::theme::font(&t.type_label),
            t.text_primary.color32(),
        )
        .size()
        .x;
    let names = widest(crate::theme::font(&t.type_body), &|i| {
        map.columns[i].header.clone()
    })
    .max(heading);
    let samples = widest(crate::theme::font(&t.type_small), &|i| {
        map.columns[i].sample.clone()
    });
    // A drop-down shows its chosen member whole ("Technical data ▸ Rated current"), its arrow beside it: measured in
    // the font and with the padding, the gap and the arrow the drop-down is drawn with.
    let none = strings::text(key!("parts-map-none")).to_owned();
    let chosen = widest(egui::TextStyle::Button.resolve(ui.style()), &|i| {
        let c = &map.columns[i];
        c.member
            .as_deref()
            .map_or_else(|| none.clone(), |m| map.member_words(m, &c.header))
    });
    let spacing = ui.spacing();
    let frame = 2.0 * spacing.button_padding.x + spacing.item_spacing.x + spacing.icon_width;
    let combo = (chosen + frame).max(t.size_dialog_label * 1.4);
    (names, samples, combo)
}

/// The column table: header, sample and the member drop-down per column, scrolling past `rows` rows.
pub(crate) fn columns(ui: &mut egui::Ui, map: &PartsMap, rows: f32, events: &mut Vec<Event>) {
    let t = crate::components::tokens(ui);
    let none = strings::text(key!("parts-map-none"));
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
    // The Sample column takes the room the Column names and the drop-downs leave in the (grown) card, so
    // a long sample is shown whole where the window has room and elides past that.
    let (names, _, combo) = column_widths(ui, map);
    let narrow = t.size_dialog_label * 0.6;
    let room = ui.available_width() - names - 2.0 * t.space_4 - ui.spacing().scroll.bar_width;
    // A drop-down never takes the First value column below its narrowest; its chosen words elide past that.
    let combo = combo.min(room - narrow).max(narrow);
    let sample = (room - combo).max(narrow);
    egui::ScrollArea::vertical()
        .id_salt("parts-map-columns")
        .max_height(t.size_control * rows)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("parts-map-grid")
                .num_columns(3)
                .spacing([t.space_4, t.space_2])
                .min_col_width(t.size_dialog_label * 0.6)
                .show(ui, |ui| {
                    ui.label(small(strings::text(key!("parts-map-column")), true));
                    ui.label(small(strings::text(key!("parts-map-sample")), true));
                    ui.label(small(strings::text(key!("parts-map-member")), true));
                    ui.end_row();
                    for (i, c) in map.columns.iter().enumerate() {
                        ui.label(crate::components::body(&t, &c.header));
                        ui.allocate_ui_with_layout(
                            Vec2::new(sample, t.size_control),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.set_width(sample);
                                ui.add(egui::Label::new(small(&c.sample, false)).truncate());
                            },
                        );
                        let mut choices = map.member_choices(&c.header);
                        if let Some(m) = &c.member
                            && !choices.contains(m)
                        {
                            choices.push(m.clone());
                        }
                        // The members in words ("Part number"); the actions keep
                        // their names.
                        let shown = c
                            .member
                            .as_deref()
                            .map_or_else(|| none.to_owned(), |m| map.member_words(m, &c.header));
                        let at = Vec2::new(combo, t.size_control);
                        let combo = ui.allocate_ui_with_layout(at, Layout::left_to_right(Align::Center), |ui| {
                        crate::components::choice(
                            ui,
                            egui::ComboBox::from_id_salt(("parts-map-member", i))
                                .selected_text(shown)
                                .width(combo)
                                .truncate(),
                            |ui| {
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
                                            map.member_words(m, &c.header),
                                        )
                                        .clicked()
                                    {
                                        events.push(Event::Action(
                                            "parts.map",
                                            json!({"column": c.header, "member": m}),
                                        ));
                                    }
                                }
                            },
                        )
                        })
                        .inner;
                        // Named "Imports as <header>" (every focusable node has a name, ADR-0014 amendment 2).
                        let name = format!("{} {}", strings::text(key!("parts-map-member")), c.header);
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
        let order = crate::dialog::footer_order(crate::dialog::Platform::host(), 1);
        for slot in order.into_iter().rev() {
            if slot == crate::dialog::Slot::Primary {
                let refusal = (!map.can_import()).then(|| map.error_text().unwrap_or_default());
                let r = button(
                    ui,
                    Kind::Primary,
                    strings::text(key!("parts-map-import")),
                    refusal.as_deref(),
                    0.0,
                );
                if r.clicked() {
                    events.push(Event::Action("parts.map_import", json!({})));
                }
            } else if button(
                ui,
                Kind::Secondary,
                strings::text(key!("parts-map-cancel")),
                None,
                0.0,
            )
            .clicked()
            {
                events.push(Event::Action("ui.dismiss", json!({})));
            }
        }
    });
}
