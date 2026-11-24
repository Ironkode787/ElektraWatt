// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's side panels in the main window's look: on the left the outline of what is edited (its lists,
//! each row a click to the thing, and what is left to do), on the right Properties (the rows of the selection, or of
//! the thing itself with nothing selected). Both paint what the view-model gives; a row's edit is one action.

use super::fields::{cancelled, committed_text, enter_pressed, new_line_key, number};
use super::label;
use crate::app::Event;
use crate::components::{self, Kind, Tip as _};
use crate::icons::IconId;
use crate::theme;
use egui::{Align, Layout, Rect, Sense, Ui, Vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::symbol_editor::{RowValue, SheetRow};
use ew_viewmodel::strings;
use serde_json::{Value, json};

/// A panel's header as the main window's panes have it (doc 16a §5.4): `size.panel_header` high, the title in
/// `type.label`.
pub(super) fn panel_header(ui: &mut Ui, title: &str) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.size_panel_header),
        Sense::hover(),
    );
    let text = if t.type_label.upper {
        title.to_uppercase()
    } else {
        title.to_owned()
    };
    ui.painter().text(
        egui::pos2(rect.left() + t.space_4, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::font(&t.type_label),
        t.text_secondary.color32(),
    );
}

/// The name of what a panel shows and what that is, under the panel's header (doc 16a §6.8).
pub(super) fn title_lines(ui: &mut Ui, title: &str, line: &str) {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_1);
    ui.add(egui::Label::new(crate::components::title(&t, title)).wrap());
    if !line.is_empty() {
        ui.add(egui::Label::new(crate::components::small_secondary(&t, line)).wrap());
    }
    ui.add_space(t.space_3);
}

/// One row of a list of the outline: `name`, and `detail` in the secondary text at its right end; `selected` paints
/// it as the tree's selected row. Named `name` for AccessKit, with its selected state.
pub(super) fn list_row(ui: &mut Ui, name: &str, detail: &str, selected: bool) -> egui::Response {
    list_row_sensing(ui, name, detail, selected, Sense::click())
}

/// [`list_row`] with what the row senses: a row of a list that is reordered by dragging senses drags too.
pub(super) fn list_row_sensing(
    ui: &mut Ui,
    name: &str,
    detail: &str,
    selected: bool,
    sense: Sense,
) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), sense);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, name));
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, theme::radius(t.radius_sm), t.accent_subtle.color32());
    }
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
    components::focus_ring(ui, &response, t.radius_sm);
    let left = rect.left() + t.size_tree_indent + t.space_2;
    // The name is what the row is: it takes its width first (up to most of the row), the detail gives way to it
    // (a part's number was cut for its description).
    let width = rect.right() - t.space_3 - left;
    let mut job = egui::text::LayoutJob::simple_singleline(
        name.to_owned(),
        theme::font(&t.type_body),
        if selected {
            t.accent.color32()
        } else {
            t.text_primary.color32()
        },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((width * 0.6).max(t.size_row));
    let galley = painter.layout_job(job);
    let mut detail_job = egui::text::LayoutJob::simple_singleline(
        detail.to_owned(),
        theme::font(&t.type_small),
        t.text_secondary.color32(),
    );
    detail_job.wrap =
        egui::text::TextWrapping::truncate_at_width((width - galley.size().x - t.space_3).max(t.size_row));
    let detail_galley = painter.layout_job(detail_job);
    let cut = galley.elided || detail_galley.elided;
    painter.galley(
        egui::pos2(left, rect.center().y - galley.size().y / 2.0),
        galley,
        t.text_primary.color32(),
    );
    painter.galley(
        egui::pos2(
            rect.right() - t.space_3 - detail_galley.size().x,
            rect.center().y - detail_galley.size().y / 2.0,
        ),
        detail_galley,
        t.text_secondary.color32(),
    );
    if cut {
        response.tip(format!("{name} \u{b7} {detail}"))
    } else {
        response
    }
}

/// A sentence of the To do list: a warning mark and the text, wrapped; a click goes to the thing it is about.
pub(super) fn todo_row(ui: &mut Ui, text: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    let width = ui.available_width();
    let left = t.size_tree_indent + t.space_2;
    let galley = ui.painter().layout(
        text.to_owned(),
        theme::font(&t.type_small),
        t.text_primary.color32(),
        width - left - t.space_3,
    );
    let height = (galley.size().y + 2.0 * t.space_2).max(t.size_row);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
    components::focus_ring(ui, &response, t.radius_sm);
    let icon = Rect::from_min_size(
        egui::pos2(rect.left() + t.space_2, rect.top() + t.space_2 + 1.0),
        Vec2::splat(t.icon_sm),
    );
    crate::icons::paint(
        ui.painter(),
        IconId::alert_triangle,
        icon,
        t.warning_icon.color32(),
        t.icon_stroke,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + left, rect.top() + t.space_2),
        galley,
        t.text_primary.color32(),
    );
    response
}

/// The line that says nothing is left to do: a green check and `text`.
pub(super) fn ready_line(ui: &mut Ui, text: &str) {
    let t = theme::current(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text));
    let icon = Rect::from_min_size(
        egui::pos2(rect.left() + t.space_2, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    crate::icons::paint(
        ui.painter(),
        IconId::circle_check,
        icon,
        t.success.color32(),
        t.icon_stroke,
    );
    ui.painter().text(
        egui::pos2(rect.left() + t.size_tree_indent + t.space_2, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::font(&t.type_body),
        t.text_primary.color32(),
    );
}

/// The width of the label column of `rows` in a panel `width` wide: the widest label, from half of
/// `size.property_label` up to 45 % of the panel (doc 16a §6.8).
fn label_width(ui: &Ui, rows: &[&SheetRow], width: f32) -> f32 {
    let t = theme::current(ui.ctx());
    let widest = rows
        .iter()
        .map(|r| {
            ui.painter()
                .layout_no_wrap(
                    r.label.clone(),
                    theme::font(&t.type_small),
                    t.text_secondary.color32(),
                )
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    (widest + t.space_3).clamp(t.size_property_label / 2.0, width * 0.45)
}

/// The words of a value that differs among the selected things.
fn varies() -> &'static str {
    label(key!("symbol-varies"))
}

/// One row of Properties: its label in the label column and its value's editor; returns the value an edit gave.
/// `owner` names what the rows belong to, for the editor's accessible name ("x (mm) of Line 1").
fn sheet_row(ui: &mut Ui, r: &SheetRow, label_w: f32, owner: &str) -> Option<Value> {
    let t = theme::current(ui.ctx());
    let name = strings::format(key!("symbol-owned"), &[&r.label, owner]);
    let mut out = None;
    let row = ui.horizontal(|ui| {
        ui.set_min_height(t.size_row);
        let (label_rect, label_response) =
            ui.allocate_exact_size(Vec2::new(label_w, t.size_row), Sense::hover());
        let mut job = egui::text::LayoutJob::simple_singleline(
            r.label.clone(),
            theme::font(&t.type_small),
            t.text_secondary.color32(),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(label_w - t.space_2);
        let galley = ui.painter().layout_job(job);
        let cut = galley.elided;
        ui.painter().galley(
            egui::pos2(label_rect.left(), label_rect.center().y - galley.size().y / 2.0),
            galley,
            t.text_secondary.color32(),
        );
        // What the row is for, where the pointer reads the label; a cut label says itself first.
        match (&r.help, cut) {
            (Some(help), true) => drop(label_response.tip(format!("{}\n{help}", r.label))),
            (Some(help), false) => drop(label_response.tip(help.clone())),
            (None, true) => drop(label_response.tip(r.label.clone())),
            (None, false) => {}
        }
        let width = ui.available_width() - t.space_3;
        ui.add_enabled_ui(r.locked.is_none(), |ui| match &r.value {
            RowValue::Text(v) => {
                let mut text = v.clone().unwrap_or_default();
                let before = text.clone();
                if committed_text(ui, ("sheet", r.key), &name, None, &mut text, width, true) && text != before
                {
                    out = Some(json!(text));
                }
            }
            RowValue::Lines(v) => {
                let mut text = v.clone().unwrap_or_default();
                if lines(ui, ("sheet", r.key), &name, &mut text, width) {
                    out = Some(json!(text));
                }
            }
            RowValue::Mm(v) | RowValue::Degrees(v) => {
                let degrees = matches!(r.value, RowValue::Degrees(_));
                match v {
                    Some(v) => {
                        let mut n = *v;
                        let (speed, range) = if degrees {
                            (1.0, -360.0..=360.0)
                        } else {
                            (0.5, -1_000_000.0..=1_000_000.0)
                        };
                        if number(ui, &mut n, speed, range, &name) {
                            out = Some(json!(n));
                        }
                    }
                    // A value that differs is typed over for all.
                    None => {
                        let id = ui.make_persistent_id(("sheet-varies", r.key));
                        let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default();
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut text)
                                .desired_width(4.0 * t.size_row)
                                .hint_text(varies()),
                        );
                        field.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), &name)
                        });
                        if field.lost_focus() {
                            if !cancelled(ui)
                                && let Ok(n) = text.trim().replace(',', ".").parse::<f64>()
                            {
                                out = Some(json!(n));
                            }
                            text.clear();
                        }
                        ui.data_mut(|d| d.insert_temp(id, text));
                    }
                }
            }
            RowValue::Choice { value, options } => {
                let shown = match value {
                    Some(word) => options
                        .iter()
                        .find(|(w, _)| w == word)
                        .map_or(word.as_str(), |(_, text)| text.as_str()),
                    None => varies(),
                };
                let mut chosen = value.clone();
                crate::components::choice(
                    ui,
                    egui::ComboBox::from_id_salt(("sheet", r.key))
                        .selected_text(shown)
                        .width(width)
                        .height(t.size_chooser_rows as f32 * 1.5 * t.size_row),
                    |ui| {
                        for (word, text) in options {
                            ui.selectable_value(&mut chosen, Some(word.clone()), text);
                        }
                    },
                )
                .response
                .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, ui.is_enabled(), &name));
                if chosen != *value
                    && let Some(word) = chosen
                {
                    out = Some(json!(word));
                }
            }
            RowValue::Flag(v) => {
                let mut on = v.unwrap_or(false);
                let words = if v.is_none() { varies() } else { "" };
                let response = ui.checkbox(&mut on, words);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), on, &name)
                });
                if response.changed() {
                    out = Some(json!(on));
                }
            }
        });
    });
    if let Some(why) = &r.locked {
        row.response.on_hover_text(why);
    }
    out
}

/// A text that may hold line breaks, committed on Enter or on losing the focus; Shift+Enter starts a new line and
/// Escape cancels. Returns whether `value` changed.
pub(super) fn lines(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    name: &str,
    value: &mut String,
    width: f32,
) -> bool {
    let id = ui.make_persistent_id(salt);
    let (source, mut buffer) = ui
        .data_mut(|d| d.get_temp::<(String, String)>(id))
        .filter(|(source, _)| source == value)
        .unwrap_or_else(|| (value.clone(), value.clone()));
    let input = id.with("input");
    let enter = enter_pressed(ui, input);
    let response = ui.add(
        egui::TextEdit::multiline(&mut buffer)
            .id(input)
            .desired_rows(1)
            .return_key(new_line_key())
            .desired_width(width),
    );
    if enter {
        response.surrender_focus();
    }
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_label(name));
    let escape = response.lost_focus() && cancelled(ui);
    let commit = (response.lost_focus() || enter) && !escape && buffer != *value && !buffer.is_empty();
    if escape || (response.lost_focus() && !commit) {
        buffer.clone_from(value);
    }
    ui.data_mut(|d| d.insert_temp(id, (source, buffer.clone())));
    if commit {
        *value = buffer;
    }
    commit
}

/// The rows of a sheet: the main ones, then the rare ones under More (closed until opened). An edit dispatches
/// `set` with the row's key and its new value.
pub(super) fn sheet_rows(
    ui: &mut Ui,
    rows: &[SheetRow],
    owner: &str,
    set: &'static str,
    events: &mut Vec<Event>,
    after_main: impl FnOnce(&mut Ui, &mut Vec<Event>),
) {
    let t = theme::current(ui.ctx());
    let width = ui.available_width();
    let show = |ui: &mut Ui, rows: Vec<&SheetRow>, events: &mut Vec<Event>| {
        let label_w = label_width(ui, &rows, width);
        for r in rows {
            ui.push_id(r.key, |ui| {
                // A field that commits as it loses the focus to a click on the bar goes before the bar's action: Save
                // and Test on a page take what was typed.
                if let Some(value) = sheet_row(ui, r, label_w, owner) {
                    events.insert(0, Event::Action(set, json!({ "key": r.key, "value": value })));
                }
            });
        }
    };
    show(ui, rows.iter().filter(|r| !r.more).collect(), events);
    after_main(ui, events);
    let more: Vec<&SheetRow> = rows.iter().filter(|r| r.more).collect();
    if !more.is_empty() {
        ui.add_space(t.space_2);
        let id = ui.make_persistent_id("symbol-sheet-more");
        if components::section_header(ui, id, label(key!("category-more")), None, false) {
            show(ui, more, events);
        }
    }
}

/// A panel's body: `add` in a scroll area with the panel's side padding.
pub(super) fn body(ui: &mut Ui, salt: &str, add: impl FnOnce(&mut Ui)) {
    let t = theme::current(ui.ctx());
    ui.spacing_mut().scroll.floating = false;
    egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: theme::margin_px(t.space_4),
                    right: theme::margin_px(t.space_3),
                    top: 0,
                    bottom: theme::margin_px(t.space_4),
                })
                .show(ui, |ui| {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(t.space_2, t.space_1);
                        add(ui);
                    });
                });
        });
}

/// A secondary button of the house.
pub(super) fn button(ui: &mut Ui, key: strings::Key<'_>) -> bool {
    components::button(ui, Kind::Secondary, label(key), true, 0.0).clicked()
}
