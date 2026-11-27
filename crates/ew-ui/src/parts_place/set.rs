// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The set an import brought into Parts (doc 07 §5.9, picture v9): its rows as the list's scope This import, the
//! middle's mapping of a table (which column is what) and what was read, and the shown row as it will be beside it.
//! A row's click sends `parts.open {row}`, a mapping change `parts.map`, the project's update `parts.import {update}`.

use egui::{RichText, ScrollArea, Sense, Ui, UiBuilder, pos2, vec2};
use ew_viewmodel::ew_api::parts_set::{SetRow, SetStatus};
use ew_viewmodel::shell::parts_place::{ImportSet, PartsPlace};
use ew_viewmodel::shell::{row_chip, row_reason, row_sentence};
use ew_viewmodel::{count_key, key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::part_dialog::elide_to;
use crate::theme;

/// Whether `row` passes the list's filters: the kind, the maker, Lacks something.
fn passes(place: &PartsPlace, row: &SetRow) -> bool {
    let kind = row.part.as_ref().map(|_| row.kind.clone());
    place
        .kind
        .as_ref()
        .is_none_or(|k| kind.as_deref() == Some(k.as_str()))
        && place.maker.as_ref().is_none_or(|m| *m == row.maker)
        && (!place.lacking || row.lacks())
}

/// The list's scope This import: the count line and the set's rows under the file's group.
pub(super) fn list(ui: &mut Ui, place: &PartsPlace, shown: &ImportSet, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let rows: Vec<(usize, &SetRow)> = shown
        .set
        .iter()
        .flat_map(|s| s.rows.iter().enumerate())
        .filter(|(_, r)| passes(place, r))
        .collect();
    let lacking = rows.iter().filter(|(_, r)| r.lacks()).count();
    ui.label(components::small_secondary(
        &t,
        strings::format(
            key!("parts-set-count-line"),
            &[
                &strings::format_count(count_key!("parts-set-rows"), rows.len()),
                &lacking.to_string(),
            ],
        ),
    ));
    ui.add_space(t.space_2);
    ScrollArea::vertical()
        .id_salt("parts-set-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let word = strings::format(key!("parts-set-group"), &[&shown.file]);
            super::list_group(ui, &word, rows.len(), false);
            for (i, row) in rows {
                if list_row(ui, row, shown.row == Some(i)).clicked() {
                    events.push(Event::Action("parts.open", json!({ "row": i })));
                }
            }
        });
}

/// One row of the set: the number (or the row's number where it has none) and the maker, the description or why it
/// is refused under them, its chip at the right and a dot before it where its part lacks something.
fn list_row(ui: &mut Ui, row: &SetRow, open: bool) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0 * t.size_row), Sense::click());
    let painter = ui.painter_at(rect);
    if open {
        painter.rect_filled(rect, theme::SQUARE, t.accent_subtle.color32());
    } else if r.hovered() {
        painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
    }
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        theme::stroke(t.border_width, t.border_subtle),
    );
    let refused = matches!(row.status, SetStatus::Refused(_));
    let x = rect.left() + t.space_2;
    let top = rect.top() + t.space_1;
    let number = if row.number.is_empty() {
        strings::format(key!("parts-set-row"), &[&row.line.to_string()])
    } else {
        row.number.clone()
    };
    let at = painter.text(
        pos2(x, top),
        egui::Align2::LEFT_TOP,
        &number,
        theme::font(&t.type_body_strong),
        if refused {
            t.text_secondary.color32()
        } else {
            t.text_primary.color32()
        },
    );
    painter.text(
        pos2(at.right() + t.space_2, top),
        egui::Align2::LEFT_TOP,
        &row.maker,
        theme::font(&t.type_body),
        t.text_secondary.color32(),
    );
    let chip = row_chip(&row.status);
    let (galley, size) = components::caption_chip_layout(ui, &t, chip);
    let chip_rect = egui::Rect::from_min_size(pos2(rect.right() - t.space_2 - size.x, top), size);
    // New in the accent's tint, refused in the warning's, the others plain (doc 16a §6.16, picture v9).
    let tint = match &row.status {
        _ if refused => (t.warning_subtle, t.warning),
        ew_viewmodel::ew_api::parts_set::SetStatus::New => (t.accent_subtle, t.accent),
        _ => (t.surface_sunken, t.text_secondary),
    };
    components::paint_tinted_chip(&painter, &t, chip_rect, galley, tint);
    if row.lacks() {
        painter.circle_filled(
            pos2(chip_rect.left() - t.space_2 - 3.5, chip_rect.center().y),
            3.5,
            t.warning.color32(),
        );
    }
    let second = match &row.status {
        SetStatus::Refused(_) => row_reason(row).unwrap_or_default(),
        _ => row.description.clone(),
    };
    let small = theme::font(&t.type_small);
    let second = elide_to(&second, rect.width() - 2.0 * t.space_2, |s| {
        components::text_width(ui, s, small.clone())
    });
    painter.text(
        pos2(x, top + t.size_row),
        egui::Align2::LEFT_TOP,
        second,
        small,
        t.text_secondary.color32(),
    );
    let name = format!("{number} {} \u{b7} {chip}", row.maker);
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, open, &name));
    r
}

/// The middle while the set shows (picture v9): the strip that says what a door into Parts is, the project's copies of
/// an EPLAN set's parts with their update, and the sheet of the read: its file, its counts, which column is what (a
/// table) and what was read.
pub(super) fn middle(ui: &mut Ui, shown: &ImportSet, width: f32, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    super::strip::note_strip(ui, strings::text(key!("parts-set-strip")));
    if let Some((held, changed)) = shown.project {
        ui.add_space(t.space_2);
        let sentence = strings::format(
            key!("parts-set-project"),
            &[&held.to_string(), &changed.to_string()],
        );
        if changed > 0 {
            super::strip::strip_with(
                ui,
                &sentence,
                strings::text(key!("parts-set-update")),
                ("parts.import", json!({ "update": true })),
                events,
            );
        } else {
            super::strip::note_strip(ui, &sentence);
        }
    }
    ui.add_space(t.space_3);
    let rect = egui::Rect::from_min_size(
        pos2(ui.max_rect().center().x - width / 2.0, ui.cursor().top()),
        vec2(width, ui.available_height()),
    );
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_lg), t.surface_panel.color32());
    let refused = shown
        .set
        .as_ref()
        .is_some_and(|s| s.rows.iter().any(|r| matches!(r.status, SetStatus::Refused(_))));
    ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(t.space_4)), |ui| {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(super::CONTENTS_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(super::CONTENTS_WIDTH);
                    ui.add_space(t.space_7);
                    if shown.map.is_some() {
                        let _ = ui.selectable_label(false, strings::text(key!("parts-set-columns")));
                    }
                    ui.horizontal(|ui| {
                        let _ = ui.selectable_label(false, strings::text(key!("parts-set-read")));
                        if refused {
                            super::dot(ui, t.warning.color32());
                        }
                    });
                },
            );
            ui.add_space(t.space_4);
            ScrollArea::vertical()
                .id_salt("parts-set-sheet")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        heading(ui, shown);
                        if let Some(map) = &shown.map {
                            columns(ui, map, events);
                        }
                        what_was_read(ui, shown);
                    });
                });
        });
    });
}

/// The heading of the read: the file, its rows and how it was read, and its counts at the right.
fn heading(ui: &mut Ui, shown: &ImportSet) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(&shown.file).font(theme::font(&t.type_title)));
            let rows = shown.set.as_ref().map_or(0, |s| s.rows.len());
            ui.label(components::body_secondary(
                &t,
                strings::format_count(count_key!("parts-set-rows"), rows),
            ));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let Some(set) = &shown.set else {
                return;
            };
            let (new, changed, same, refused) = set.counts();
            // Right to left: refused last, so that it stands at the right edge.
            let plain = (t.surface_sunken, t.text_secondary);
            for (n, k, tint) in [
                (refused, key!("parts-set-refused"), (t.warning_subtle, t.warning)),
                (same, key!("parts-set-same"), plain),
                (changed, key!("parts-set-changed"), plain),
                (new, key!("parts-set-new"), (t.accent_subtle, t.accent)),
            ] {
                if n > 0 {
                    components::tinted_chip(ui, &strings::format(k, &[&n.to_string()]), tint);
                }
            }
        });
    });
    ui.add_space(t.space_3);
}

/// Which column is what (a table): the source key the map is saved as, the columns with their first value and the field
/// each goes to, what the shared sizes do, and the map's refusal.
fn columns(ui: &mut Ui, map: &ew_viewmodel::shell::PartsMap, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    crate::parts_sheet::group_heading(
        ui,
        strings::text(key!("parts-set-columns-head")),
        Some(strings::text(key!("parts-set-columns-line"))),
        None,
    );
    ui.horizontal(|ui| {
        let label = ui.label(components::body_secondary(
            &t,
            strings::text(key!("parts-set-saved-as-label")),
        ));
        let id = egui::Id::unique("parts-set-source");
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
            components::caption_badge(ui, strings::text(key!("parts-map-saved")));
        }
    });
    ui.label(components::small_secondary(
        &t,
        strings::text(key!("parts-set-saved-as")),
    ));
    ui.add_space(t.space_2);
    // The sheet scrolls; the table is shown whole.
    crate::parts_map::columns(ui, map, f32::INFINITY, events);
    ui.add_space(t.space_2);
    ui.add(
        egui::Label::new(components::small_secondary(
            &t,
            strings::text(key!("parts-set-sizes")),
        ))
        .wrap(),
    );
    if let Some(e) = map.error_text() {
        components::live_line(ui, &e, true);
    }
}

/// What was read: a sentence for every row refused, the same or a repeat; a line that every row makes a part.
fn what_was_read(ui: &mut Ui, shown: &ImportSet) {
    let t = theme::current(ui.ctx());
    crate::parts_sheet::group_heading(ui, strings::text(key!("parts-set-read")), None, None);
    let lines: Vec<(bool, String)> = shown
        .set
        .iter()
        .flat_map(|s| &s.rows)
        .filter_map(|r| row_sentence(r).map(|l| (matches!(r.status, SetStatus::Refused(_)), l)))
        .collect();
    if lines.is_empty() {
        ui.label(components::body_secondary(
            &t,
            strings::text(key!("parts-set-all-read")),
        ));
    }
    for (refused, line) in lines {
        ui.horizontal_top(|ui| {
            if refused {
                super::mark(ui, crate::icons::IconId::alert_triangle, t.warning.color32());
            } else {
                super::dash(ui);
            }
            ui.add(egui::Label::new(RichText::new(line)).wrap());
        });
    }
}

/// The right column while the set shows: the shown row as it will be (its kind and its fields) and its Ready for.
pub(super) fn as_it_will_be(ui: &mut Ui, shown: &ImportSet) {
    let t = theme::current(ui.ctx());
    let Some(row) = shown.shown() else {
        return;
    };
    if row.part.is_none() {
        return;
    }
    egui::Frame::NONE
        .fill(t.surface_panel.color32())
        .stroke(theme::stroke(t.border_width, t.border_subtle))
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::same(t.space_4 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(&row.number).font(theme::font(&t.type_body_strong)));
            let kind = ew_viewmodel::shell::part_dialog::part_kind_word(&row.kind);
            ui.label(components::small_secondary(
                &t,
                strings::format(key!("parts-set-as-it-will-be"), &[&kind]),
            ));
            ui.add_space(t.space_3);
            egui::Grid::new("parts-set-fields")
                .num_columns(2)
                .spacing([t.space_4, t.space_2])
                .show(ui, |ui| {
                    for (path, text) in &shown.texts {
                        let word = ew_viewmodel::ew_api::part_fields::field(path)
                            .map(|f| strings::render(&ew_viewmodel::ew_api::part_fields::word(f.word)))
                            .unwrap_or_default();
                        ui.label(components::body_secondary(&t, word));
                        ui.label(components::body(&t, text));
                        ui.end_row();
                    }
                });
        });
    ui.add_space(t.space_4);
    super::ready_for(
        ui,
        &strings::format(key!("parts-set-ready-for"), &[&row.number]),
        &shown.ready,
    );
}
