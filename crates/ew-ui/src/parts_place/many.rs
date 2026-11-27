// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Several parts selected in Parts (doc 07 §5.9): one sheet of the fields they share, a value they all have shown as
//! it is and one they differ in as "mixed"; a value typed goes to all of them (`parts.field`). Beside it the parts
//! selected, each with where it lives.

use egui::{RichText, ScrollArea, Ui, UiBuilder, pos2, vec2};
use ew_viewmodel::ew_api::output::FieldType;
use ew_viewmodel::shell::parts_many::ManySheet;
use ew_viewmodel::{key, strings};

use crate::app::Event;
use crate::components;
use crate::theme;

/// The middle: the heading of how many parts, the line of what typing does, the lock line where one is not his, and
/// the shared fields two to a line.
pub(super) fn middle(ui: &mut Ui, many: &ManySheet, width: f32, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let rect = egui::Rect::from_min_size(
        pos2(ui.max_rect().center().x - width / 2.0, ui.cursor().top()),
        vec2(width, ui.available_height()),
    );
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_lg), t.surface_panel.color32());
    ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(t.space_4)), |ui| {
        ScrollArea::vertical()
            .id_salt("parts-many-sheet")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let count = many.sheets.len().to_string();
                ui.label(RichText::new(strings::format(key!("parts-many-title"), &[&count])).font(theme::font(&t.type_title)));
                let numbers: Vec<String> = many.numbers().into_iter().map(|(n, _)| n).collect();
                ui.label(components::body_secondary(&t, numbers.join(" \u{b7} ")));
                ui.add_space(t.space_2);
                ui.add(egui::Label::new(components::small_secondary(&t, strings::format(key!("parts-many-line"), &[&count]))).wrap());
                let not_his = many.not_his();
                if !not_his.is_empty() {
                    ui.add_space(t.space_2);
                    egui::Frame::NONE
                        .fill(t.surface_sunken.color32())
                        .corner_radius(theme::radius(t.radius_md))
                        .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let why = strings::render(&strings::keyed(
                                key!("reason-parts-many-not-yours"),
                                vec![ew_viewmodel::ew_api::TextArg::Plain(not_his.join(", "))],
                            ));
                            ui.add(egui::Label::new(why).wrap());
                            // Make my own from each: that part opened alone, as his own new part.
                            ui.horizontal_wrapped(|ui| {
                                for (param, (sheet, lives)) in many.parts.iter().zip(&many.sheets) {
                                    if !matches!(lives, ew_viewmodel::ew_api::parts::Lives::NotMine { .. }) {
                                        continue;
                                    }
                                    let number = sheet.text("part_number");
                                    let words = strings::format(key!("parts-many-own"), &[&number]);
                                    if ui.button(words).clicked() {
                                        events.push(Event::Action("parts.open", serde_json::json!({ "part": param })));
                                        events.push(Event::Action("parts.own", serde_json::json!({})));
                                    }
                                }
                            });
                        });
                }
                ui.add_space(t.space_3);
                let editable = not_his.is_empty();
                let mixed = strings::text(key!("parts-many-mixed"));
                // The Connections table's cells stand in its own table below.
                let cells: Vec<crate::parts_sheet::Cell<'_>> = many
                    .fields
                    .iter()
                    .filter(|(field, _)| !field.contains('['))
                    .map(|(field, text)| {
                        let label = many.words(field);
                        // As the sheet of one part writes it: a length with its unit after the value, a closed list
                        // of words as a choice.
                        let ty = many.sheets.first().and_then(|(s, _)| s.row(field)).map(|r| &r.ty);
                        let nm = ty.is_some_and(|ty| matches!(ty.inner(), FieldType::Nm));
                        let unit = nm.then(|| strings::text(key!("parts-unit-mm")));
                        let choices: Option<Vec<String>> = ty.and_then(|ty| match ty.inner() {
                            FieldType::Enum { options } => Some(
                                ty.is_optional()
                                    .then(String::new)
                                    .into_iter()
                                    .chain(options.iter().cloned())
                                    .collect(),
                            ),
                            _ => None,
                        });
                        let cell: crate::parts_sheet::Cell<'_> = Box::new(move |ui, ev| {
                            let v = crate::parts_sheet::Value {
                                field,
                                label: &label,
                                text: text.as_deref().unwrap_or_default(),
                                unit,
                                help: None,
                                under: None,
                                editable,
                                focus: false,
                            };
                            let enter = match &choices {
                                Some(choices) => {
                                    let words = |w: &str| super::choice_words(field, w);
                                    if let Some(c) = crate::parts_sheet::choice(ui, &v, choices, words) {
                                        ev.push(Event::Action(
                                            "parts.field",
                                            serde_json::json!({ "field": field, "value": c }),
                                        ));
                                    }
                                    false
                                }
                                None => crate::parts_sheet::text_value(ui, v, ev),
                            };
                            // A field they differ in: "mixed" under it, never a value of its own.
                            if text.is_none() {
                                let t = theme::current(ui.ctx());
                                ui.horizontal(|ui| {
                                    ui.add_space(crate::parts_sheet::LABEL + t.space_3);
                                    ui.label(components::small_tertiary(&t, mixed));
                                });
                            }
                            enter
                        });
                        cell
                    })
                    .collect();
                let _ = crate::parts_sheet::grid(ui, cells, events);
                // Where they have the same pins: the sheet's own Connections table.
                if let Some((first, table)) = shared_table(many) {
                    let group = ew_viewmodel::ew_api::part_fields::Group::Connections;
                    crate::parts_sheet::group_heading(
                        ui,
                        strings::text(strings::Key::run_time(group.word())),
                        Some(strings::text(key!("parts-many-cells"))),
                        None,
                    );
                    super::connections_table(ui, first, &table, editable, events);
                }
                if let Some(e) = &many.error {
                    ui.add_space(t.space_3);
                    ui.label(RichText::new(e).color(t.error.color32()));
                }
            });
    });
}

/// The Connections table the parts selected share (doc 07 §5.9), as the first part's with each cell's shared text; a
/// cell they differ in is empty. `None` where their pins differ.
fn shared_table(many: &ManySheet) -> Option<(&ew_viewmodel::ew_api::PartSheet, ew_viewmodel::ew_api::DraftTable)> {
    let (first, _) = many.sheets.first()?;
    let mut table = first.table("cp_pattern")?.clone();
    let prefix = format!("{}[", table.path);
    if !many.fields.iter().any(|(f, _)| f.starts_with(&prefix)) {
        return None;
    }
    for (i, row) in table.rows.iter_mut().enumerate() {
        for ((member, _), text) in table.columns.iter().zip(row.iter_mut()) {
            let field = format!("{}[{i}].{member}", table.path);
            *text = many
                .fields
                .iter()
                .find(|(f, _)| *f == field)
                .and_then(|(_, t)| t.clone())
                .unwrap_or_default();
        }
    }
    Some((first, table))
}

/// The right column: the parts selected, each by its number and where it lives.
pub(super) fn parts(ui: &mut Ui, many: &ManySheet) {
    let t = theme::current(ui.ctx());
    ui.label(RichText::new(strings::text(key!("parts-many-parts"))).font(theme::font(&t.type_label)));
    ui.add_space(t.space_2);
    for (number, lives) in many.numbers() {
        ui.horizontal(|ui| {
            ui.label(components::body_strong(&t, number));
            components::caption_chip(ui, &lives);
        });
    }
}
