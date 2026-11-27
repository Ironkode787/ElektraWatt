// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Several parts selected in Parts (doc 07 §5.9): one sheet of the fields they share, a value they all have shown as
//! it is and one they differ in as "mixed"; a value typed goes to all of them (`parts.field`). Beside it the parts
//! selected, each with where it lives.

use egui::{RichText, ScrollArea, Ui, UiBuilder, pos2, vec2};
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
                ui.label(
                    RichText::new(strings::format_count(ew_viewmodel::count_key!("parts-many-title"), many.sheets.len()))
                        .font(theme::font(&t.type_title)),
                );
                let numbers: Vec<String> = many.numbers().into_iter().map(|(n, _)| n).collect();
                ui.label(components::body_secondary(&t, numbers.join(" \u{b7} ")));
                ui.add_space(t.space_2);
                ui.add(
                    egui::Label::new(components::small_secondary(
                        &t,
                        strings::format(key!("parts-many-line"), &[&count]),
                    ))
                    .wrap(),
                );
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
                                    let words = strings::format(key!("parts-many-own"), &[number]);
                                    if ui.button(words).clicked() {
                                        events.push(Event::Action(
                                            "parts.open",
                                            serde_json::json!({ "part": param }),
                                        ));
                                        events.push(Event::Action("parts.own", serde_json::json!({})));
                                    }
                                }
                            });
                        });
                }
                ui.add_space(t.space_3);
                let editable = not_his.is_empty();
                let mixed = strings::text(key!("parts-many-mixed"));
                groups(ui, many, editable, mixed, events);
                if let Some(e) = &many.error {
                    ui.add_space(t.space_3);
                    ui.label(RichText::new(e).color(t.error.color32()));
                }
            });
    });
}

/// The fields the parts share under the headings of a single part's sheet, in its order, each drawn by the cell that
/// draws it there: a value they all have, or `mixed` as its greyed placeholder; the Connections table where they have
/// the same pins.
fn groups(ui: &mut Ui, many: &ManySheet, editable: bool, mixed: &str, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::part_fields::Group;
    let Some((first, _)) = many.sheets.first() else {
        return;
    };
    let shown = |f: &str| many.fields.iter().find(|(x, _)| x == f).map(|(_, t)| t);
    // The declared rows they share, as the first part's rows with the shared text.
    let rows: Vec<(ew_viewmodel::ew_api::DraftRow, bool)> = first
        .rows
        .iter()
        .filter_map(|r| {
            let text = shown(r.path)?;
            let mut row = r.clone();
            row.text = text.clone().unwrap_or_default();
            Some((row, text.is_none()))
        })
        .collect();
    for group in first.sections() {
        let hint = |is_mixed: bool| is_mixed.then_some(mixed);
        let mut cells: Vec<crate::parts_sheet::Cell<'_>> = Vec::new();
        match group {
            Group::Identity => {
                for (f, text) in many
                    .fields
                    .iter()
                    .filter(|(f, _)| ew_viewmodel::ew_api::identity_fields().any(|i| i == f.as_str()))
                {
                    let Some(f) = ew_viewmodel::ew_api::identity_fields().find(|i| *i == f.as_str()) else {
                        continue;
                    };
                    let label = ew_viewmodel::shell::part_dialog::draft_label(f);
                    let text = (text.as_deref().unwrap_or_default(), hint(text.is_none()));
                    cells.push(super::identity_cell(text, None, f, label, editable, false));
                }
            }
            Group::Technical => {
                for (f, text) in &many.fields {
                    if let Some(k) = f.strip_prefix("tech:") {
                        let text = (text.as_deref().unwrap_or_default(), hint(text.is_none()));
                        cells.push(super::tech_cell(first, k, text, editable));
                    }
                }
            }
            _ => {
                for (row, is_mixed) in rows.iter().filter(|(r, _)| r.group == group) {
                    cells.push(super::row_cell(None, (row, hint(*is_mixed)), editable, None));
                }
            }
        }
        let table = (group == Group::Connections)
            .then(|| shared_table(many))
            .flatten();
        if cells.is_empty() && table.is_none() {
            continue;
        }
        let line = strings::lookup(&format!("part-line-{}", group.name().replace('_', "-")));
        crate::parts_sheet::group_heading(
            ui,
            strings::text(strings::Key::run_time(group.word())),
            line,
            None,
        );
        let _ = crate::parts_sheet::grid(ui, cells, events);
        if let Some((sheet, table)) = table {
            ui.label(components::small_secondary(
                &theme::current(ui.ctx()),
                strings::text(key!("parts-many-cells")),
            ));
            super::pins::connections_table(ui, sheet, &table, editable, events);
        }
    }
}

/// The Connections table the parts selected share (doc 07 §5.9), as the first part's with each cell's shared text; a
/// cell they differ in is empty. `None` where their pins differ.
fn shared_table(
    many: &ManySheet,
) -> Option<(&ew_viewmodel::ew_api::PartSheet, ew_viewmodel::ew_api::DraftTable)> {
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
