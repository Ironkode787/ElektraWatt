// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rails (WP-S2.17 part 3, D10; doc 16a §5.4 and §5.6): the navigator rail at the window's left with History at its
//! foot, the palettes rail at its right, both views of `ew_viewmodel::rail::rail`.
//!
//! A rail button is a 32-px icon button (`radius.md`, the icon at `icon.md` in `color.text.secondary`); the active
//! entry takes `color.accent.subtle`, an accent icon and a 2-px accent indicator bar 4 px left of the button. Every
//! button is a named `Button` node with its key, so every navigator is one click and one name away (FT-509). Ctrl+1 to
//! Ctrl+9 select the left rail's entries by position.

use egui::{Id, Key, Modifiers, Rect, Sense, Ui, pos2, vec2};
use ew_viewmodel::rail::{RailButton, RailModel, RailTarget, Side};
use ew_viewmodel::shell::ShellModel;
use serde_json::{Value, json};

use crate::app::Event;
use crate::command_bar::{Spec, button_response, paint_icon, state_overlay};
use crate::theme::{self, Tokens};

/// The id of the rail button `word` on `side` (tests find the buttons by name; the id keeps them apart).
fn button_id(side: Side, word: &str) -> Id {
    Id::unique(("ew-rail", side.word(), word))
}

/// The event a click on `b` sends.
fn click(side: Side, b: &RailButton) -> Event {
    match b.target {
        RailTarget::Pane(_) => Event::Action("rail.show", json!({ "side": side.word(), "pane": b.word })),
        RailTarget::Action(action) => Event::Action(action, Value::Null),
    }
}

/// Paints rail button `b` at `rect`; a click pushes its event.
fn button(ui: &Ui, t: &Tokens, side: Side, b: &RailButton, rect: Rect, events: &mut Vec<Event>) {
    let response = button_response(
        ui,
        button_id(side, b.word),
        rect,
        Spec {
            name: &b.label,
            shortcut: &b.shortcut,
            enabled: true,
            checked: Some(b.active),
            sense: Sense::click(),
        },
    );
    let painter = ui.painter();
    if b.active {
        painter.rect_filled(rect, theme::radius(t.radius_md), t.accent_subtle.color32());
        // The indicator bar: 2 px, 4 px left of the button, inset 8 px top and bottom (doc 16a §5.4).
        let bar = Rect::from_min_max(
            pos2(rect.left() - t.space_2 - t.radius_mark, rect.top() + t.space_3),
            pos2(rect.left() - t.space_2, rect.bottom() - t.space_3),
        );
        painter.rect_filled(bar, theme::radius(t.radius_mark), t.accent.color32());
    }
    state_overlay(ui, t, &response, true, t.radius_md);
    let colour = if b.active { t.accent } else { t.text_secondary };
    let icon = Rect::from_center_size(rect.center(), vec2(t.icon_md, t.icon_md));
    paint_icon(ui, b.icon, icon, colour.color32(), t.icon_stroke);
    if response.has_focus() {
        painter.rect_stroke(
            rect,
            theme::radius(t.radius_md),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Inside,
        );
    }
    let tip = if b.shortcut.is_empty() {
        b.label.clone()
    } else {
        format!("{} ({})", b.label, crate::keys::display(&b.shortcut))
    };
    let response = response.on_hover_text(tip);
    if response.clicked() {
        events.push(click(side, b));
    }
}

/// A rail's strip at `rect`: the chrome fill and the 1-px line on its inner side.
fn strip(ui: &Ui, t: &Tokens, side: Side, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_none), t.surface_chrome.color32());
    let x = match side {
        Side::Left => rect.right() - t.border_width / 2.0,
        Side::Right => rect.left() + t.border_width / 2.0,
    };
    painter.vline(x, rect.y_range(), theme::stroke(t.border_width, t.border_default));
}

/// The top of the `i`-th button of a rail starting at `top`.
fn slot(t: &Tokens, rect: Rect, i: usize) -> Rect {
    let x = rect.center().x - t.size_rail_button / 2.0;
    let y = rect.top() + t.space_3 + i as f32 * (t.size_rail_button + t.space_2);
    Rect::from_min_size(pos2(x, y), vec2(t.size_rail_button, t.size_rail_button))
}

/// Paints the left rail into `ui` (a side panel of `size.rail`): the navigator entries from the top, History at the
/// foot.
pub(crate) fn show_left(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let model: RailModel = ew_viewmodel::rail::rail(shell);
    let rect = ui.max_rect();
    strip(ui, &t, Side::Left, rect);
    for (i, b) in model.left.iter().enumerate() {
        button(ui, &t, Side::Left, b, slot(&t, rect, i), events);
    }
    let foot = Rect::from_min_size(
        pos2(
            rect.center().x - t.size_rail_button / 2.0,
            rect.bottom() - t.space_3 - t.size_rail_button,
        ),
        vec2(t.size_rail_button, t.size_rail_button),
    );
    button(ui, &t, Side::Left, &model.foot, foot, events);
    ui.allocate_rect(rect, Sense::hover());
}

/// Paints the right rail into `ui`: the palettes.
pub(crate) fn show_right(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let model = ew_viewmodel::rail::rail(shell);
    let rect = ui.max_rect();
    strip(ui, &t, Side::Right, rect);
    for (i, b) in model.right.iter().enumerate() {
        button(ui, &t, Side::Right, b, slot(&t, rect, i), events);
    }
    ui.allocate_rect(rect, Sense::hover());
}

/// Ctrl+1 to Ctrl+9 (doc 16a §5.4): the left rail's entry at that position; `true` when a key was taken.
pub(crate) fn keys(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) -> bool {
    const DIGITS: [Key; 9] = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    let Some(i) = ctx.input_mut(|input| {
        DIGITS
            .iter()
            .position(|k| input.consume_key(Modifiers::COMMAND, *k))
    }) else {
        return false;
    };
    let model = ew_viewmodel::rail::rail(shell);
    if let Some(b) = model.left.get(i) {
        events.push(click(Side::Left, b));
    }
    true
}
