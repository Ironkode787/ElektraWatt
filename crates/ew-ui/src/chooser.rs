// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Place tool's Join chooser, the chooser list of doc 16a §6.22: a popup of `color.surface.raised`,
//! `elevation.1`, `radius.md`, `space.2` padding and `size.chooser` wide, with no title; rows of `size.row` with a
//! 16-px kind icon in `color.text.secondary`, the printed name in `type.body` and the kind word right-aligned in
//! `type.small` `color.text.tertiary`; the active row has the hover overlay; a last line of key chips. It opens 8 px
//! from the pointer's 16-px box, inside the page view and above the HUD.
//!
//! Keys: Up and Down (wrapping, `chooser.row`), Home, End, type-ahead on the name (`chooser.type`, the typed text
//! kept for a second), Enter picks (`chooser.choose`), Escape or a click beside it closes it (`ui.dismiss`, the
//! topmost popup). Every
//! change is a registry action, so the headless mode drives the same model. AccessKit: a `ListBox` named by the
//! question ("Join device"), its rows `ListBoxOption`s with position and set size; the active row is selected.

use egui::{Id, Key, Modifiers, Rect, vec2};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::ChooserValue;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, text_width};
use crate::icons::IconId;
use crate::theme;

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
    // The keys (doc 16a §6.22): consumed before anything else sees them.
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
    let rows = n.min(t.size_chooser_rows as usize);
    let max_height = t.size_row * rows as f32;
    // 8 px from the pointer's 16-px box (doc 16a §6.22), else the page view's centre.
    let at = ctx
        .pointer_latest_pos()
        .filter(|p| bound.contains(*p))
        .map_or(bound.center(), |p| p + vec2(16.0, 16.0));
    let list_id = Id::unique("join-chooser");
    // A click beside it closes it, as Escape does.
    crate::components::overlay_backdrop(ctx, list_id, false, events);
    let question = ch.question.clone();
    egui::Area::new(list_id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain_to(bound)
        .show(ctx, |ui| {
            let (name_font, kind_font) = (theme::font(&t.type_body), theme::font(&t.type_small));
            let widest = ch
                .rows
                .iter()
                .map(|r| {
                    text_width(ui, &r.name, name_font.clone())
                        + if r.kind.is_empty() {
                            0.0
                        } else {
                            text_width(ui, &r.kind, kind_font.clone()) + t.space_4
                        }
                })
                .fold(0.0_f32, f32::max);
            let content = t.space_2 + t.icon_sm + t.space_2 + widest + 2.0 * t.space_2;
            let width = (content + 2.0 * (t.space_2 + t.border_width))
                .clamp(t.size_chooser_min_width, t.size_chooser_max_width)
                .min(bound.width());
            components::popover_frame(&t).show(ui, |ui| {
                let inner = width - 2.0 * (t.space_2 + t.border_width);
                ui.set_width(inner);
                let name = |i: usize| {
                    let row = &ch.rows[i];
                    if row.kind.is_empty() {
                        row.name.clone()
                    } else {
                        format!("{}, {}", row.name, row.kind)
                    }
                };
                let paint = |ui: &egui::Ui, i: usize, r: Rect| {
                    let row = &ch.rows[i];
                    let glyph = match row.value {
                        ChooserValue::NewDevice => IconId::plus,
                        ChooserValue::Join { .. } | ChooserValue::JoinRep { .. } => {
                            IconId::circuit_switch_open
                        }
                    };
                    components::chooser_row(ui, &t, r, glyph, &row.name, &row.kind);
                };
                let sizes = (inner, max_height, t.size_row);
                if let Some(i) = components::chooser_list(
                    ui,
                    &t,
                    list_id,
                    &question,
                    sizes,
                    (n, Some(ch.row)),
                    name,
                    paint,
                ) {
                    events.push(Event::Action("chooser.choose", json!({ "row": i })));
                }
                components::chooser_keys(ui, &t);
            });
        });
}
