// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The components of the sheet of Parts (doc 16a §6.27), each built once: the fields' grid, a sheet field (a value
//! with its unit, read as text until it is changed), a group's heading with its one line. The sheet in
//! [`crate::parts_place`] is made of them.

use egui::{Align, Layout, RichText, Sense, TextEdit, Ui, vec2};
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;
use crate::theme;

/// The label's width of a sheet field (doc 16a §6.27).
pub(crate) const LABEL: f32 = 150.0;

/// The narrowest a column of the fields' grid gets before the grid takes one column.
const COLUMN_MIN: f32 = 330.0;

/// One cell of the fields' grid: it draws a field and says whether Enter was pressed in it.
pub(crate) type Cell<'a> = Box<dyn FnOnce(&mut Ui, &mut Vec<Event>) -> bool + 'a>;

/// The fields' grid (doc 16a §6.27): the cells two to a line (one where the sheet is too narrow for two), each line as
/// high as its highest cell. `true` when Enter was pressed in a field.
pub(crate) fn grid(ui: &mut Ui, cells: Vec<Cell<'_>>, events: &mut Vec<Event>) -> bool {
    let t = theme::current(ui.ctx());
    let gap = t.space_5;
    let width = ui.available_width();
    let cols = if width >= 2.0 * COLUMN_MIN + gap { 2 } else { 1 };
    let cell_w = (width - gap * (cols as f32 - 1.0)) / cols as f32;
    let mut enter = false;
    let mut cells = cells.into_iter().peekable();
    while cells.peek().is_some() {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for _ in 0..cols {
                let Some(cell) = cells.next() else {
                    break;
                };
                ui.allocate_ui_with_layout(vec2(cell_w, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(cell_w);
                    enter |= cell(ui, events);
                });
            }
        });
        ui.add_space(t.space_1);
    }
    enter
}

/// What stands under a field's value: what follows while it is empty (a warning), or a text that cannot be read (an
/// error).
pub(crate) enum Under {
    /// What follows while the field is empty.
    Lacks(String),
    /// Why the typed text cannot be read.
    Error(String),
}

/// A sheet field's text value.
pub(crate) struct Value<'a> {
    /// The field `parts.field` names.
    pub field: &'a str,
    /// Its label.
    pub label: &'a str,
    /// Its text.
    pub text: &'a str,
    /// The unit written after the value (`mm`), not in the label.
    pub unit: Option<&'a str>,
    /// What the datum is for: the field's tooltip.
    pub help: Option<&'a str>,
    /// What stands under it.
    pub under: Option<Under>,
    /// The greyed placeholder of an empty value (several parts selected: "mixed", which typing replaces).
    pub hint: Option<&'a str>,
    /// Whether the person may change it; else plain text.
    pub editable: bool,
    /// Whether it takes the keyboard focus.
    pub focus: bool,
}

/// The label of a field, `LABEL` wide, its explanation as its tooltip; the rest of the line is the value's.
pub(crate) fn label(ui: &mut Ui, text: &str, help: Option<&str>) -> egui::Response {
    let t = theme::current(ui.ctx());
    let r = ui
        .allocate_ui_with_layout(
            vec2(LABEL, t.size_control),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_size(vec2(LABEL, t.size_control));
                ui.add(egui::Label::new(RichText::new(text).color(t.text_secondary.color32())).truncate())
            },
        )
        .inner;
    match help {
        Some(h) => r.tip(h),
        None => r,
    }
}

/// A sheet field (doc 16a §6.27): its label, its value read as text with a hairline under it until it is being
/// changed (the focus ring while it is), its unit after it, and under it what follows while it is empty or why the
/// text cannot be read. A part that is not the person's own shows plain text. `true` when Enter was pressed in it.
pub(crate) fn text_value(ui: &mut Ui, v: Value<'_>, events: &mut Vec<Event>) -> bool {
    let field = v.field;
    let (changed, enter) = line(ui, v.label, (v.text, v.hint), v.unit, v.help, v.editable, v.focus);
    if let Some(text) = changed {
        events.push(Event::Action(
            "parts.field",
            json!({ "field": field, "value": text }),
        ));
    }
    under(ui, v.under, v.editable);
    enter
}

/// The line of a sheet field: the label, the value (an edit without a frame, a hairline under it, the focus ring while
/// it is changed; plain text where `editable` is off) and the unit after it. The text typed, when it changed, and
/// whether Enter was pressed in it.
pub(crate) fn line(
    ui: &mut Ui,
    label_text: &str,
    (value, hint): (&str, Option<&str>),
    unit: Option<&str>,
    help: Option<&str>,
    editable: bool,
    focus: bool,
) -> (Option<String>, bool) {
    let t = theme::current(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = label(ui, label_text, help);
        ui.add_space(t.space_3);
        if !editable {
            let shown = match (value.trim(), unit) {
                ("", _) => hint.unwrap_or("\u{2014}").to_owned(),
                (x, Some(u)) => format!("{x} {u}"),
                (x, None) => x.to_owned(),
            };
            ui.add(egui::Label::new(shown).wrap());
            return (None, false);
        }
        let unit_w = unit.map_or(0.0, |u| {
            ui.fonts_mut(|f| {
                f.layout_no_wrap(
                    u.to_owned(),
                    theme::font(&t.type_body),
                    t.text_secondary.color32(),
                )
                .size()
                .x
            }) + t.space_2
        });
        let mut text = value.to_owned();
        let edit = ui
            .add(
                TextEdit::singleline(&mut text)
                    .hint_text(hint.unwrap_or_default())
                    .frame(egui::Frame::NONE)
                    .desired_width((ui.available_width() - unit_w).max(t.size_control))
                    .min_size(vec2(0.0, t.size_control)),
            )
            .labelled_by(label.id);
        edit.widget_info(|| {
            let mut info = egui::WidgetInfo::text_edit(true, value, &text, "");
            info.label = Some(label_text.to_owned());
            info
        });
        let rect = edit.rect;
        if edit.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                theme::radius(t.radius_sm),
                theme::stroke(t.focus_width, t.focus),
                egui::StrokeKind::Outside,
            );
        } else {
            let colour = if edit.hovered() {
                t.border_default
            } else {
                t.border_subtle
            };
            ui.painter().hline(
                rect.x_range(),
                rect.bottom(),
                theme::stroke(t.border_width, colour),
            );
        }
        if let Some(u) = unit {
            ui.add_space(t.space_2);
            ui.label(RichText::new(u).color(t.text_secondary.color32()));
        }
        if focus {
            edit.request_focus();
        }
        let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ((text != value).then_some(text), enter)
    })
    .inner
}

/// A sheet field that is a choice: `v`'s label and a combo box of `choices` by their words, `v.field` its ID; plain
/// words where `v.editable` is off. The value picked, when it changed.
pub(crate) fn choice(
    ui: &mut Ui,
    v: &Value<'_>,
    choices: &[String],
    word: impl Fn(&str) -> String,
) -> Option<String> {
    let t = theme::current(ui.ctx());
    let (id, label_text, value, help, editable) = (v.field, v.label, v.text, v.help, v.editable);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        label(ui, label_text, help);
        ui.add_space(t.space_3);
        if !editable {
            ui.add(egui::Label::new(word(value)).wrap());
            return None;
        }
        let mut picked = None;
        // An empty value of several parts that differ reads as its greyed placeholder.
        let shown = match v.hint.filter(|_| value.is_empty()) {
            Some(h) => RichText::new(h).color(t.text_tertiary.color32()),
            None => RichText::new(word(value)),
        };
        let combo = crate::components::choice(
            ui,
            egui::ComboBox::from_id_salt(id)
                .selected_text(shown)
                .width(ui.available_width()),
            |ui| {
                for c in choices {
                    if ui.selectable_label(c == value, word(c)).clicked() {
                        picked = Some(c.clone());
                    }
                }
            },
        )
        .response;
        combo.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, label_text));
        picked.filter(|c| c != value)
    })
    .inner
}
/// A field or a table ringed in the warning colour (doc 07 §5.7): the one an outcome's Edit part… opened the sheet at,
/// while the part lacks it; `body` drawn plainly where `on` is off.
pub(crate) fn ringed<R>(ui: &mut Ui, on: bool, body: impl FnOnce(&mut Ui) -> R) -> R {
    if !on {
        return body(ui);
    }
    let t = theme::current(ui.ctx());
    let inner = egui::Frame::NONE
        .inner_margin(egui::Margin::same(t.space_1 as i8))
        .show(ui, body);
    ui.painter().rect_stroke(
        inner.response.rect,
        theme::radius(t.radius_md),
        theme::stroke(t.focus_width, t.warning),
        egui::StrokeKind::Outside,
    );
    inner.inner
}

// What stands under a field: a warning or an error line with its icon, indented to the value. A field that can be
// changed (`reserve`) keeps the warning's line whether or not it has something to say, and the warning stays on that
// one line (its whole sentence in its tooltip, and under Ready for), so the fields and buttons below do not move as a
// warning comes and goes; an error, which stands while the typed text cannot be read, wraps.
pub(crate) fn under(ui: &mut Ui, under: Option<Under>, reserve: bool) {
    let t = theme::current(ui.ctx());
    let Some(u) = under else {
        if reserve {
            let h = ui
                .fonts_mut(|f| f.row_height(&theme::font(&t.type_small)))
                .max(t.icon_sm);
            ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        }
        return;
    };
    let (text, colour, icon, one_line) = match u {
        Under::Lacks(s) => (s, t.warning, crate::icons::IconId::alert_triangle, reserve),
        Under::Error(s) => (s, t.error, crate::icons::IconId::alert_circle, false),
    };
    ui.horizontal_top(|ui| {
        ui.add_space(LABEL + t.space_3);
        let (rect, _) = ui.allocate_exact_size(vec2(t.icon_sm, t.icon_sm), Sense::hover());
        crate::icons::paint(ui.painter(), icon, rect, colour.color32(), t.icon_stroke);
        let label = egui::Label::new(
            RichText::new(&text)
                .font(theme::font(&t.type_small))
                .color(colour.color32()),
        );
        if one_line {
            ui.add(label.truncate()).on_hover_text(&text);
        } else {
            ui.add(label.wrap());
        }
    });
}

/// A group's heading (doc 16a §6.27): a hairline and `space.4` above it, the group's word in `type.body.strong` with
/// its one line of explanation beside it, the longer explanation `tip` as that line's tooltip.
pub(crate) fn group_heading(ui: &mut Ui, word: &str, line: Option<&str>, tip: Option<&str>) {
    let t = theme::current(ui.ctx());
    ui.add_space(t.space_4);
    let at = ui.available_rect_before_wrap();
    ui.painter().hline(
        at.x_range(),
        at.top(),
        theme::stroke(t.border_width, t.border_subtle),
    );
    ui.add_space(t.space_3);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(word).font(theme::font(&t.type_body_strong)));
        if let Some(line) = line {
            ui.add_space(t.space_3);
            let r = ui.label(
                RichText::new(line)
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            );
            if let Some(tip) = tip {
                r.tip(tip);
            }
        }
    });
    ui.add_space(t.space_2);
}
