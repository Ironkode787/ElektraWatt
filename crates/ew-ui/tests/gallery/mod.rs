// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The component gallery of WP-S2.17 §5 item 7: the parts the tokens style, in their states, on one surface per theme
//! (buttons rest, hovered-like selected, disabled and primary; a field; navigator-like rows with the cursor and a
//! marked row; chips; a menu-like list; a dialog card with its elevation; a toast; the severity words). Everything is
//! drawn from the tokens of the theme on show; nothing here is a design decision of its own.

use egui::{Align2, Pos2, Rect, RichText, Sense, Ui, Vec2};
use ew_ui::theme::{self, Density, Theme, Tokens};

/// Shows the gallery in `theme` (a theme word of the settings) at Comfortable density.
pub fn show(ui: &mut Ui, theme_word: &str) {
    let which = Theme::ALL
        .into_iter()
        .find(|t| t.word() == theme_word)
        .expect("a theme word");
    let tokens = theme::tokens(which, Density::Comfortable);
    if theme::current(ui.ctx()).theme != which {
        theme::apply(&tokens, ui.ctx());
        ui.ctx().request_repaint();
    }
    let t = &tokens;
    let full = ui.max_rect();
    ui.painter()
        .rect_filled(full, theme::SQUARE, t.surface_chrome.color32());
    ui.spacing_mut().item_spacing = Vec2::splat(t.space_2);
    egui::Frame::new()
        .fill(t.surface_panel.color32())
        .inner_margin(t.space_4)
        .show(ui, |ui| {
            ui.label(RichText::new(format!("Gallery: {theme_word}")).font(theme::font(&t.type_title)));
            ui.label(
                RichText::new("Body text, secondary text and tertiary text.").font(theme::font(&t.type_body)),
            );
            ui.label(RichText::new("Secondary").color(t.text_secondary.color32()));
            ui.label(RichText::new("Tertiary").color(t.text_tertiary.color32()));
            buttons(ui, t);
            field(ui);
            rows(ui, t);
            chips(ui, t);
            ui.horizontal_top(|ui| {
                menu(ui, t);
                dialog(ui, t);
            });
            toast(ui, t);
            severities(ui, t);
        });
}

fn buttons(ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        let _ = ui.button("Rest");
        let _ = ui.selectable_label(true, "Selected");
        ui.add_enabled(false, egui::Button::new("Disabled"));
        let _ = ui.add(
            egui::Button::new(RichText::new("Primary").color(t.text_on_accent.color32()))
                .fill(t.accent.color32()),
        );
        let _ = ui.checkbox(&mut true, "Checked");
        let _ = ui.radio(false, "Radio");
    });
}

fn field(ui: &mut Ui) {
    let mut text = String::from("-3W1");
    ui.horizontal(|ui| {
        let label = ui.label("Device tag");
        ui.text_edit_singleline(&mut text).labelled_by(label.id);
    });
}

fn rows(ui: &mut Ui, t: &Tokens) {
    let width = ui.available_width().min(360.0);
    for (i, (label, selected)) in [("Pages", false), ("101 Supply", true), ("102 Controls", false)]
        .into_iter()
        .enumerate()
    {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, t.size_row), Sense::hover());
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, theme::radius(t.radius_sm), t.accent_subtle.color32());
        }
        let indent = t.size_tree_indent * i.min(1) as f32;
        painter.text(
            Pos2::new(rect.left() + t.space_3 + indent, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            theme::font(&t.type_body),
            t.text_primary.color32(),
        );
        // A selected row draws its tertiary parts in text.secondary (doc 16a §4.15).
        let tail = if selected {
            t.text_secondary
        } else {
            t.text_tertiary
        };
        painter.text(
            Pos2::new(rect.right() - t.space_3, rect.center().y),
            Align2::RIGHT_CENTER,
            "QET grid",
            theme::font(&t.type_small),
            tail.color32(),
        );
    }
}

fn chips(ui: &mut Ui, t: &Tokens) {
    ui.horizontal(|ui| {
        for (word, fill) in [
            ("All", t.accent_subtle),
            ("Errors", t.error_subtle),
            ("Warnings", t.warning_subtle),
        ] {
            let galley = ui.painter().layout_no_wrap(
                word.into(),
                theme::font(&t.type_small),
                t.text_primary.color32(),
            );
            let size = Vec2::new(galley.size().x + 2.0 * t.space_3, t.size_chip);
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            ui.painter().rect(
                rect,
                theme::radius(t.radius_pill),
                fill.color32(),
                theme::stroke(t.border_width, t.border_default),
                egui::StrokeKind::Inside,
            );
            ui.painter().galley(
                rect.center() - galley.size() / 2.0,
                galley,
                t.text_primary.color32(),
            );
        }
    });
}

fn card(ui: &mut Ui, t: &Tokens, size: Vec2, elevation: &theme::Elevation) -> Rect {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .add(theme::shadow(elevation).as_shape(rect, theme::radius(t.radius_lg)));
    ui.painter().rect(
        rect,
        theme::radius(t.radius_lg),
        t.surface_raised.color32(),
        theme::stroke(t.border_raised_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    rect
}

fn menu(ui: &mut Ui, t: &Tokens) {
    let rect = card(
        ui,
        t,
        Vec2::new(180.0, 3.0 * t.size_row + 2.0 * t.space_1),
        &t.elevation_1,
    );
    for (i, (item, hovered)) in [("Open...", false), ("Save", true), ("Export PDF...", false)]
        .into_iter()
        .enumerate()
    {
        let top = rect.top() + t.space_1 + i as f32 * t.size_row;
        let row = Rect::from_min_size(
            Pos2::new(rect.left() + t.space_1, top),
            Vec2::new(rect.width() - 2.0 * t.space_1, t.size_row),
        );
        if hovered {
            ui.painter().rect_filled(
                row,
                theme::radius(t.radius_sm),
                t.state_hover.over(t.surface_raised).color32(),
            );
        }
        ui.painter().text(
            Pos2::new(row.left() + t.space_3, row.center().y),
            Align2::LEFT_CENTER,
            item,
            theme::font(&t.type_body),
            t.text_primary.color32(),
        );
    }
}

fn dialog(ui: &mut Ui, t: &Tokens) {
    let rect = card(ui, t, Vec2::new(300.0, 120.0), &t.elevation_2);
    let painter = ui.painter();
    painter.text(
        rect.left_top() + Vec2::splat(t.space_4),
        Align2::LEFT_TOP,
        "Delete the page?",
        theme::font(&t.type_title),
        t.text_primary.color32(),
    );
    painter.text(
        rect.left_top() + Vec2::new(t.space_4, t.space_4 + t.type_title.line + t.space_2),
        Align2::LEFT_TOP,
        "It can be undone.",
        theme::font(&t.type_body),
        t.text_secondary.color32(),
    );
    let button = Rect::from_min_size(
        rect.right_bottom() - Vec2::new(t.space_4 + t.size_button_min_width, t.space_4 + t.size_control),
        Vec2::new(t.size_button_min_width, t.size_control),
    );
    painter.rect_filled(button, theme::radius(t.radius_md), t.accent.color32());
    painter.text(
        button.center(),
        Align2::CENTER_CENTER,
        "Delete",
        theme::font(&t.type_label),
        t.text_on_accent.color32(),
    );
}

fn toast(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(t.size_toast.min(ui.available_width()), t.size_control + t.space_3),
        Sense::hover(),
    );
    ui.painter()
        .add(theme::shadow(&t.elevation_2).as_shape(rect, theme::radius(t.radius_md)));
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_md), t.surface_inverse.color32());
    ui.painter().text(
        Pos2::new(rect.left() + t.space_3, rect.center().y),
        Align2::LEFT_CENTER,
        "Saved project",
        theme::font(&t.type_body),
        t.text_on_inverse.color32(),
    );
}

fn severities(ui: &mut Ui, t: &Tokens) {
    // Severity reads without colour: each row has its word next to its colour (§4.5).
    ui.horizontal(|ui| {
        for (word, colour) in [("error", t.error), ("warning", t.warning_icon), ("info", t.info)] {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(t.size_severity_icon), Sense::hover());
            ui.painter().circle_filled(
                rect.center(),
                t.size_severity_icon / 2.0 - t.space_icon / 2.0,
                colour.color32(),
            );
            ui.label(word);
        }
    });
}
