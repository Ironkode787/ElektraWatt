// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Text styles dialog's own content (doc 16 §3.7, doc 16a §6.12): one table of the project's text styles under the
//! headings of what they print, a row per style with its name and where it prints, its typeface, its height (typed, or
//! one of the standard heights), bold, a sample in the drawing's own face at twice its size on paper, and Reset; the quiet
//! note of a height under the readable minimum under the row's name. Every control dispatches a `text_styles.*`
//! action: one undo step each, shown on the open page at once.

use egui::{RichText, TextEdit, Ui};
use ew_viewmodel::ew_api::Typeface;
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::text_styles::{
    GROUPS, STANDARD_HEIGHTS, TextStyleRow, TextStylesDialog, TypefaceLine, faces, group_words,
};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind, Tip as _};
use crate::theme;

/// Points per millimetre of em height of a sample: twice the size on paper on a 96-dpi screen, so that the smallest
/// styles stay readable on the page; every row keeps the proportion of its height to the others.
const PX_PER_MM: f32 = 2.0 * 96.0 / 25.4;

/// The largest sample, in points: a cover title keeps the row readable.
const MAX_SAMPLE: f32 = 44.0;

/// A text a style prints, as its sample: what a person sees in that style on a drawing.
fn sample(key: &str) -> &'static str {
    match key {
        "tag.main" => "-1Q1",
        "data.small" => "Motor 1.5 kW",
        "cp.small" => "13 14 A1",
        "pin" => "1 2 3",
        "xref.device" => "102.3 ∞",
        "marker" => "101 102",
        "terminal" => "1 2 3",
        "cover.title" | "cover.subtitle" | "section.subtitle" => "Control cabinet",
        "section.id" => "A1",
        _ => "Text 123",
    }
}

/// A line in `type.small`, `colour`, wrapped.
fn small(ui: &mut Ui, t: &theme::Tokens, text: &str, colour: egui::Color32) {
    ui.add(egui::Label::new(RichText::new(text).font(theme::font(&t.type_small)).color(colour)).wrap());
}

/// The Text styles dialog's content.
pub(crate) fn text_styles(
    ui: &mut Ui,
    shell: &ShellModel,
    d: &TextStylesDialog,
    just_opened: bool,
    events: &mut Vec<Event>,
) {
    let t = components::tokens(ui);
    let rows = shell.text_style_rows();
    let writable = shell.is_writable();
    let line = shell.typeface_line();
    if let Some(line) = &line {
        typeface_line(ui, &t, line, writable, events);
    }
    // The samples of the Standard styles in the project's typeface, once egui has it.
    let typeface = line
        .and_then(|l| l.face)
        .and_then(|face| crate::fonts::typeface_family(ui.ctx(), face));
    // The table takes the room the window has: at 2560 × 1440 it shows more rows, not a strip beside empty space.
    let room = ui.ctx().content_rect().height() * 0.62;
    egui::ScrollArea::vertical()
        .max_height(room)
        .min_scrolled_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for group in GROUPS {
                let in_group: Vec<&TextStyleRow> = rows.iter().filter(|r| r.group == group).collect();
                if in_group.is_empty() {
                    continue;
                }
                ui.add_space(t.space_3);
                ui.label(
                    RichText::new(group_words(group))
                        .font(theme::font(&t.type_body_strong))
                        .color(t.text_primary.color32()),
                );
                ui.separator();
                for row in in_group {
                    let focused = d.focus.as_deref() == Some(row.key.as_str());
                    let frame = egui::Frame::new()
                        .fill(if focused {
                            t.accent_subtle.color32()
                        } else {
                            egui::Color32::TRANSPARENT
                        })
                        .corner_radius(theme::radius(t.radius_md))
                        .inner_margin(egui::Margin::symmetric(
                            theme::margin_px(t.space_2),
                            theme::margin_px(t.space_1),
                        ));
                    let shown = frame.show(ui, |ui| style_row(ui, &t, row, (writable, typeface.as_ref()), events));
                    if focused && just_opened {
                        shown.response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            }
        });
    ui.add_space(t.space_2);
    small(
        ui,
        &t,
        strings::text(key!("text-styles-faces-note")),
        t.text_secondary.color32(),
    );
    if let Some(note) = &d.note {
        small(ui, &t, note, t.text_primary.color32());
    }
}

/// The *Standard typeface* line (doc 09 §4.8): what prints the Standard typeface, Choose a font file… and Use Noto Sans,
/// and the note that the project carries the file.
fn typeface_line(
    ui: &mut Ui,
    t: &theme::Tokens,
    line: &TypefaceLine,
    writable: bool,
    events: &mut Vec<Event>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        ui.label(
            RichText::new(strings::text(key!("text-styles-typeface-title")))
                .font(theme::font(&t.type_body_strong))
                .color(t.text_primary.color32()),
        );
        ui.label(RichText::new(&line.words).color(t.text_primary.color32()));
        let choose = components::button(
            ui,
            Kind::Secondary,
            strings::text(key!("text-styles-typeface-choose")),
            writable,
            0.0,
        );
        if choose.clicked() {
            events.push(Event::Action("text_styles.typeface", json!({})));
        }
        let clear = components::button(
            ui,
            Kind::Subtle,
            strings::text(key!("text-styles-typeface-clear")),
            writable && line.own,
            0.0,
        );
        if clear.clicked() {
            events.push(Event::Action("text_styles.typeface_clear", json!({})));
        }
    });
    small(
        ui,
        t,
        strings::text(key!("text-styles-typeface-note")),
        t.text_secondary.color32(),
    );
}

/// One style: its name and where it prints, its typeface, height, bold, sample and Reset; `typeface` is the egui family of the project's typeface, which the samples of the Standard styles are drawn in.
fn style_row(
    ui: &mut Ui,
    t: &theme::Tokens,
    row: &TextStyleRow,
    (writable, typeface): (bool, Option<&egui::FontFamily>),
    events: &mut Vec<Event>,
) {
    let name_width = (ui.available_width() * 0.34).max(t.size_dialog_label);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        ui.allocate_ui_with_layout(
            egui::vec2(name_width, t.size_control),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(name_width);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new(&row.name).color(t.text_primary.color32()));
                small(ui, t, &row.prints, t.text_secondary.color32());
                if let Some(note) = &row.small {
                    small(ui, t, note, t.warning.color32());
                }
            },
        );
        ui.add_enabled_ui(writable, |ui| {
            face_combo(ui, t, row, events);
            height_field(ui, t, row, events);
            // A face without a bold cut: the toggle is off and says why.
            let has_bold = row.face.has_bold();
            let mut bold = ui
                .add_enabled(
                    has_bold,
                    egui::Button::selectable(
                        row.bold,
                        RichText::new(strings::text(key!("text-styles-bold-letter"))).strong(),
                    ),
                )
                .tip(strings::text(key!("text-styles-bold")));
            if !has_bold {
                bold = bold.disabled_tip(strings::text(key!("text-styles-bold-none")));
            }
            if bold.clicked() {
                events.push(Event::Action(
                    "text_styles.set",
                    json!({ "style": row.key, "bold": !row.bold }),
                ));
            }
        });
        // The sample in proportion to its size on paper, capped for the largest styles.
        let em_mm = row.em.0 as f32 / 1_000_000.0;
        let size = (em_mm * PX_PER_MM).clamp(4.0, MAX_SAMPLE);
        let sample_width = (ui.available_width() - 90.0).max(60.0);
        ui.allocate_ui_with_layout(
            egui::vec2(sample_width, t.size_control),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_width(sample_width);
                ui.add(
                    egui::Label::new(
                        RichText::new(sample(&row.key))
                            .font(egui::FontId::new(
                                size,
                                match (typeface, &row.face) {
                                    (Some(family), Typeface::Standard | Typeface::Unknown(_)) => family.clone(),
                                    _ => crate::fonts::drawing_family(&row.face, row.bold),
                                },
                            ))
                            .color(t.text_primary.color32()),
                    )
                    .truncate(),
                );
            },
        );
        let reset = components::button(
            ui,
            Kind::Subtle,
            strings::text(key!("text-styles-reset")),
            writable && row.changed,
            0.0,
        )
        .tip(strings::text(key!("text-styles-reset-tip")));
        if reset.clicked() {
            events.push(Event::Action("text_styles.reset", json!({ "style": row.key })));
        }
    });
}

/// The typeface list of a row.
fn face_combo(ui: &mut Ui, t: &theme::Tokens, row: &TextStyleRow, events: &mut Vec<Event>) {
    let list = faces();
    let shown = list
        .iter()
        .find(|(f, _)| *f == row.face)
        .map_or("", |(_, w)| w.as_str());
    let combo = crate::components::choice(
        ui,
        egui::ComboBox::from_id_salt(("text-style-face", &row.key))
            .selected_text(shown)
            .width(t.size_dialog_label * 0.9),
        |ui| {
            for (face, words) in &list {
                if ui.selectable_label(*face == row.face, words.as_str()).clicked() && *face != row.face {
                    events.push(Event::Action(
                        "text_styles.set",
                        json!({ "style": row.key, "face": face.word() }),
                    ));
                }
            }
        },
    );
    combo.response.tip(strings::text(key!("text-styles-typeface")));
}

/// The height of a row: a field a person types in (Enter or leaving it sets it), and the standard heights beside it.
fn height_field(ui: &mut Ui, t: &theme::Tokens, row: &TextStyleRow, events: &mut Vec<Event>) {
    let id = egui::Id::unique(("text-style-height", &row.key));
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| row.height.clone());
    let edit = ui
        .add(
            TextEdit::singleline(&mut text)
                .desired_width(t.size_control * 2.2)
                .min_size(egui::vec2(0.0, t.size_control)),
        )
        .tip(strings::text(key!("text-styles-height")));
    if edit.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if edit.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if text.trim() != row.height {
            events.push(Event::Action(
                "text_styles.set",
                json!({ "style": row.key, "height": text.trim() }),
            ));
        }
    }
    ui.label(RichText::new(strings::text(key!("choice-mm"))).color(t.text_secondary.color32()));
    let menu = crate::components::choice(
        ui,
        egui::ComboBox::from_id_salt(("text-style-heights", &row.key))
            .selected_text("")
            .width(t.size_control),
        |ui| {
            for h in STANDARD_HEIGHTS {
                let words = format!("{} mm", strings::decimal(h));
                let on = row.height == strings::decimal(h);
                if ui.selectable_label(on, words).clicked() && !on {
                    events.push(Event::Action(
                        "text_styles.set",
                        json!({ "style": row.key, "height": h }),
                    ));
                }
            }
        },
    );
    menu.response
        .tip(strings::text(key!("text-styles-standard-heights")));
}
