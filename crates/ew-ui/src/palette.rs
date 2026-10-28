// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command palette (WP-S2.01 part 1, spec section 5.3; FR-UI-012 part-1 subset), restyled by WP-S2.17 part 4 (doc
//! 16a §6.14). Ctrl+K opens a popover with a search field over the registry actions the window can run (the shell
//! leaves out what needs parameters, P3); matching is a case-insensitive substring on the label and the ID; disabled
//! entries follow the enabled ones with their reason; Enter runs the selected enabled entry; Escape closes it.
//!
//! **Look.** No title: `size.palette` wide (560 px), anchored under the command bar's search field (or centred at the
//! window's top fifth when opened by key), `color.surface.raised`, `elevation.2`, `radius.lg`; a 36-px search field
//! with a search icon; result rows of `size.palette_row` (32 px): the action's icon, the label with the matched
//! characters in `type.body.strong`, the breadcrumb in `color.text.tertiary` ("Draw · Place"), the key as key chips at
//! the right; the selected row on the hover overlay; a disabled row in `color.text.disabled` with its reason in
//! `color.text.secondary`.
//!
//! **Accessibility.** Each row is a button named by its label (and its reason when disabled), carrying its shortcut.

use egui::text::LayoutJob;
use egui::{Align, Align2, Id, Key, Layout, Modifiers, Rect, ScrollArea, Sense, TextEdit, Vec2};
use ew_viewmodel::actions::PaletteEntry;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::icons::{self, IconId};
use crate::theme::{self, Tokens};

/// The label of `entry` with the characters that match `query` (case-insensitively, the first occurrence) in
/// `type.body.strong`.
fn label_job(t: &Tokens, label: &str, query: &str, colour: egui::Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal = egui::TextFormat::simple(theme::font(&t.type_body), colour);
    let strong = egui::TextFormat::simple(theme::font(&t.type_body_strong), colour);
    let lower = label.to_lowercase();
    let q = query.to_lowercase();
    match (!q.is_empty() && lower.len() == label.len())
        .then(|| lower.find(&q))
        .flatten()
    {
        Some(at) => {
            job.append(&label[..at], 0.0, normal.clone());
            job.append(&label[at..at + q.len()], 0.0, strong);
            job.append(&label[at + q.len()..], 0.0, normal);
        }
        None => job.append(label, 0.0, normal),
    }
    job
}

/// One result row; returns whether it was clicked.
fn row(ui: &mut egui::Ui, t: &Tokens, entry: &PaletteEntry, query: &str, selected: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, t.size_palette_row), Sense::click());
    let name = match &entry.reason {
        Some(reason) if !entry.enabled => format!("{}. {reason}", entry.label),
        _ => entry.label.clone(),
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, entry.enabled, &name));
    if !entry.shortcut.is_empty() {
        let shortcut = crate::keys::display(&entry.shortcut);
        ui.ctx()
            .accesskit_node_builder(response.id, |n| n.set_keyboard_shortcut(shortcut));
    }
    let painter = ui.painter();
    if selected || (entry.enabled && response.hovered()) {
        painter.rect_filled(rect, theme::radius(t.radius_sm), t.state_hover.color32());
    }
    let (text, icon_colour) = if entry.enabled {
        (t.text_primary, t.text_secondary)
    } else {
        (t.text_disabled, t.text_disabled)
    };
    let mut x = rect.left() + t.space_3;
    let icon = match &entry.run {
        ew_viewmodel::actions::PaletteRun::Action(id) => ew_viewmodel::actions::find(id)
            .and_then(|m| m.icon)
            .and_then(IconId::from_key),
        ew_viewmodel::actions::PaletteRun::Command(_) => None,
    };
    if let Some(id) = icon {
        let r = Rect::from_center_size(
            egui::pos2(x + t.icon_sm / 2.0, rect.center().y),
            Vec2::splat(t.icon_sm),
        );
        icons::paint(painter, id, r, icon_colour.color32(), t.icon_stroke);
    }
    x += t.icon_sm + t.space_3;
    let galley = painter.layout_job(label_job(t, &entry.label, query, text.color32()));
    let label_w = galley.size().x;
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        text.color32(),
    );
    x += label_w + t.space_3;
    let (tail, tail_colour) = match &entry.reason {
        Some(reason) if !entry.enabled => (reason.clone(), t.text_secondary),
        _ => (entry.breadcrumb.clone(), t.text_tertiary),
    };
    let tail = painter.layout_no_wrap(tail, theme::font(&t.type_small), tail_colour.color32());
    painter.galley(
        egui::pos2(x, rect.center().y - tail.size().y / 2.0),
        tail,
        tail_colour.color32(),
    );
    if !entry.shortcut.is_empty() {
        let chip_area = Rect::from_min_max(
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right() - t.space_3, rect.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(chip_area)
                .layout(Layout::right_to_left(Align::Center)),
        );
        crate::components::key_chip(&mut child, &entry.shortcut);
    }
    response.clicked() && entry.enabled
}

/// Paints the palette when it is open. `just_opened` puts the keyboard focus into its search field. `anchor` is the
/// command bar's search field (WP-S2.17 part 2, doc 16a §5.2): the palette opens under it, right-aligned with it.
pub fn show(
    ctx: &egui::Context,
    shell: &ShellModel,
    just_opened: bool,
    anchor: Option<Rect>,
    events: &mut Vec<Event>,
) {
    let Some(state) = shell.palette() else {
        return;
    };
    // The keys of the list are consumed before the text field sees them.
    let (down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        events.push(Event::PaletteMove(1));
    }
    if up {
        events.push(Event::PaletteMove(-1));
    }
    if enter {
        events.push(Event::PaletteRun);
    }
    let t = theme::current(ctx);
    let entries = shell.palette_entries();
    let area = egui::Area::new(Id::unique("command-palette")).order(egui::Order::Foreground);
    let area = match anchor {
        Some(field) => area
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(field.right_bottom() + Vec2::new(0.0, t.space_1)),
        None => area.pivot(Align2::CENTER_TOP).fixed_pos(egui::pos2(
            ctx.content_rect().center().x,
            ctx.content_rect().height() / 5.0,
        )),
    };
    area.show(ctx, |ui| {
        egui::Frame::new()
            .fill(t.surface_raised.color32())
            .stroke(theme::stroke(t.border_raised_width, t.border_default))
            .corner_radius(theme::radius(t.radius_lg))
            .shadow(theme::shadow(&t.elevation_2))
            .inner_margin(egui::Margin::same(t.space_2 as i8))
            .show(ui, |ui| {
                ui.set_width(t.size_palette - 2.0 * t.space_2);
                ui.spacing_mut().item_spacing.y = t.space_1;
                // The 36-px search field with its icon.
                let field_h = t.size_control + t.space_3;
                ui.horizontal(|ui| {
                    ui.set_min_height(field_h);
                    ui.add_space(t.space_2);
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_sm), Sense::hover());
                    icons::paint(
                        ui.painter(),
                        IconId::search,
                        r,
                        t.text_secondary.color32(),
                        t.icon_stroke,
                    );
                    let mut query = state.query.clone();
                    let edit = ui.add(
                        TextEdit::singleline(&mut query)
                            .hint_text(strings::text("palette-hint"))
                            .frame(egui::Frame::NONE)
                            .font(theme::font(&t.type_body))
                            .desired_width(f32::INFINITY),
                    );
                    if just_opened {
                        edit.request_focus();
                    }
                    if query != state.query {
                        events.push(Event::PaletteQuery(query));
                    }
                });
                let (line, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), t.border_width), Sense::hover());
                ui.painter()
                    .rect_filled(line, theme::SQUARE, t.border_subtle.color32());
                if entries.is_empty() {
                    ui.add_space(t.space_3);
                    ui.horizontal(|ui| {
                        ui.add_space(t.space_3);
                        ui.label(
                            egui::RichText::new(strings::text("palette-no-match"))
                                .color(t.text_secondary.color32()),
                        );
                    });
                    ui.add_space(t.space_3);
                }
                ScrollArea::vertical()
                    .max_height(t.size_palette_row * 10.0)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, entry) in entries.iter().enumerate() {
                            if row(ui, &t, entry, &state.query, i == state.selected) {
                                events.push(Event::PaletteClick(i));
                            }
                        }
                    });
            });
    });
}
