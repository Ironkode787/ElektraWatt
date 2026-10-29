// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Place tool's Join chooser (WP-S2.18 D2), the chooser list of doc 16a §6.22: a popup of `color.surface.raised`,
//! `elevation.1`, `radius.md`, `space.2` padding and `size.chooser` wide, with no title; rows of `size.row` with a
//! 16-px kind icon in `color.text.secondary`, the printed name in `type.body` and the kind word right-aligned in
//! `type.small` `color.text.tertiary`; the active row has the hover overlay; a last line of key chips. It opens 8 px
//! from the pointer's 16-px box, inside the page view and above the HUD.
//!
//! Keys: Up and Down (wrapping, `chooser.row`), Home, End, type-ahead on the name (`chooser.type`, the typed text
//! kept for a second), Enter picks (`chooser.choose`), Escape closes it (`ui.dismiss`, the topmost popup). Every
//! change is a registry action, so the headless mode drives the same model. AccessKit: a `ListBox` named by the
//! question ("Join device"), its rows `ListBoxOption`s with position and set size; the active row is selected.

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{FontId, Id, Key, Modifiers, Rect, Sense, Ui, pos2, vec2};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::ChooserValue;
use serde_json::json;

use crate::app::Event;
use crate::icons::{self, IconId};
use crate::theme::{self, Rgba};

/// How long typed characters stay one type-ahead text, seconds.
const TYPE_AHEAD_S: f64 = 1.0;

/// The type-ahead text the window keeps between frames.
#[derive(Default, Debug)]
pub(crate) struct TypeAhead {
    text: String,
    at: f64,
}

/// Paints the Join chooser when it is open, inside the page view `pane` above the HUD band; consumes its keys.
pub(crate) fn show(
    ctx: &egui::Context,
    shell: &ShellModel,
    pane: Option<Rect>,
    typing: &mut TypeAhead,
    events: &mut Vec<Event>,
) {
    let Some(ch) = shell.chooser() else {
        typing.text.clear();
        return;
    };
    let n = ch.rows.len();
    // The keys (§6.22): consumed before anything else sees them.
    let (down, up, home, end, enter, typed, now) = ctx.input_mut(|i| {
        let down = i.consume_key(Modifiers::NONE, Key::ArrowDown);
        let up = i.consume_key(Modifiers::NONE, Key::ArrowUp);
        let home = i.consume_key(Modifiers::NONE, Key::Home);
        let end = i.consume_key(Modifiers::NONE, Key::End);
        let enter = i.consume_key(Modifiers::NONE, Key::Enter);
        let mut typed = String::new();
        i.events.retain(|e| match e {
            // The canvas chords (R, X, J) stay silent while the chooser is open: the shell's key routing treats it as
            // a popup (`app.rs`), so a letter is text here.
            egui::Event::Text(t) => {
                typed.push_str(t);
                false
            }
            _ => true,
        });
        (down, up, home, end, enter, typed, i.time)
    });
    for (hit, params) in [
        (down, json!({"step": 1})),
        (up, json!({"step": -1})),
        (home, json!({"to": 0})),
        (end, json!({"to": -1})),
    ] {
        if hit {
            events.push(Event::Action("chooser.row", params));
        }
    }
    if !typed.is_empty() {
        if now - typing.at > TYPE_AHEAD_S {
            typing.text.clear();
        }
        typing.text.push_str(&typed);
        typing.at = now;
        events.push(Event::Action("chooser.type", json!({ "text": typing.text })));
    }
    if enter {
        events.push(Event::Action("chooser.choose", json!({})));
        return;
    }
    if n == 0 {
        return;
    }
    let t = theme::current(ctx);
    let bound = pane.unwrap_or_else(|| ctx.content_rect());
    // Above the HUD band at the page view's bottom (§7.4: the HUD is an obstacle).
    let bound = Rect::from_min_max(
        bound.min,
        pos2(
            bound.right(),
            (bound.bottom() - crate::hud::fit_inset(&t)).max(bound.top()),
        ),
    );
    let name_font = theme::font(&t.type_body);
    let kind_font = theme::font(&t.type_small);
    let widest = ch
        .rows
        .iter()
        .map(|r| {
            text_width(ctx, &r.name, name_font.clone())
                + if r.kind.is_empty() {
                    0.0
                } else {
                    text_width(ctx, &r.kind, kind_font.clone()) + t.space_4
                }
        })
        .fold(0.0_f32, f32::max);
    let content = t.space_2 + t.icon_sm + t.space_2 + widest + 2.0 * t.space_2;
    let width = (content + 2.0 * (t.space_2 + t.border_width))
        .clamp(t.size_chooser_min_width, t.size_chooser_max_width)
        .min(bound.width());
    let rows = n.min(t.size_chooser_rows as usize);
    let max_height = t.size_row * rows as f32;
    // 8 px from the pointer's 16-px box (§6.22), else the page view's centre.
    let at = ctx
        .pointer_latest_pos()
        .filter(|p| bound.contains(*p))
        .map_or(bound.center(), |p| p + vec2(16.0, 16.0));
    let list_id = Id::unique("join-chooser");
    let question = ch.question.clone();
    egui::Area::new(list_id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain_to(bound)
        .show(ctx, |ui| {
            crate::components::card_frame(&t)
                .fill(t.surface_raised.color32())
                .corner_radius(theme::radius(t.radius_md))
                .shadow(theme::shadow(&t.elevation_1))
                .inner_margin(egui::Margin::same(t.space_2 as i8))
                .show(ui, |ui| {
                    let inner = width - 2.0 * (t.space_2 + t.border_width);
                    ui.set_width(inner);
                    let list_rect = Rect::from_min_size(ui.max_rect().min, vec2(inner, max_height));
                    let list = ui.interact(list_rect, list_id, Sense::hover());
                    list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, &question));
                    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("join-chooser-scroll")
                            .max_height(max_height)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for (i, row) in ch.rows.iter().enumerate() {
                                    let selected = i == ch.row;
                                    let (r, response) = ui.allocate_exact_size(
                                        vec2(ui.available_width(), t.size_row),
                                        Sense::click(),
                                    );
                                    let name = if row.kind.is_empty() {
                                        row.name.clone()
                                    } else {
                                        format!("{}, {}", row.name, row.kind)
                                    };
                                    response.widget_info(|| {
                                        egui::WidgetInfo::selected(
                                            egui::Role::ListBoxOption,
                                            true,
                                            selected,
                                            &name,
                                        )
                                    });
                                    ui.ctx().accesskit_node_builder(response.id, |node| {
                                        node.set_selected(selected);
                                        node.set_position_in_set(i + 1);
                                        node.set_size_of_set(n);
                                    });
                                    // The hover overlay, as menus (§6.13), never the accent.
                                    if selected || response.hovered() {
                                        ui.painter().rect_filled(
                                            r,
                                            theme::radius(t.radius_sm),
                                            t.state_hover.color32(),
                                        );
                                    }
                                    let icon = Rect::from_center_size(
                                        pos2(r.left() + t.space_2 + t.icon_sm / 2.0, r.center().y),
                                        vec2(t.icon_sm, t.icon_sm),
                                    );
                                    let glyph = match row.value {
                                        ChooserValue::NewDevice => IconId::plus,
                                        ChooserValue::Join { .. } => IconId::circuit_switch_open,
                                    };
                                    icons::paint(
                                        ui.painter(),
                                        glyph,
                                        icon,
                                        t.text_secondary.color32(),
                                        t.icon_stroke,
                                    );
                                    let kind = text_width(ui.ctx(), &row.kind, kind_font.clone());
                                    let text_rect = Rect::from_min_max(
                                        pos2(icon.right() + t.space_2, r.top()),
                                        pos2(r.right() - kind - 2.0 * t.space_2, r.bottom()),
                                    );
                                    paint_text(ui, text_rect, &row.name, name_font.clone(), t.text_primary);
                                    let kind_rect = Rect::from_min_max(
                                        pos2(r.right() - kind - t.space_2, r.top()),
                                        pos2(r.right() - t.space_2, r.bottom()),
                                    );
                                    paint_text(ui, kind_rect, &row.kind, kind_font.clone(), t.text_tertiary);
                                    if selected {
                                        response.scroll_to_me(None);
                                    }
                                    if response.clicked() {
                                        events.push(Event::Action("chooser.choose", json!({ "row": i })));
                                    }
                                }
                            });
                    });
                    ui.add_space(t.space_2);
                    ui.horizontal(|ui| {
                        for chip in ["\u{2191} \u{2193}", "Enter", "Esc"] {
                            crate::components::key_chip(ui, chip);
                        }
                    });
                });
        });
}

/// Paints `text` left-aligned and vertically centred in `rect`, truncated to its width.
fn paint_text(ui: &Ui, rect: Rect, text: &str, font: FontId, colour: Rgba) {
    let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::simple(font, colour.color32()));
    job.wrap = TextWrapping::truncate_at_width(rect.width().max(0.0));
    let galley = ui.painter().layout_job(job);
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, colour.color32());
}

/// The unwrapped width of `text` in `font`.
fn text_width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
    ctx.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, theme::NONE).size().x)
}
