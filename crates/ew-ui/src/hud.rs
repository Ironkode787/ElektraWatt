// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool HUD and the zoom control over the canvas (WP-S2.17 part 2, doc 16a §5.5), from
//! `ew_viewmodel::command_bar::hud`. The HUD is a pill centred `space.5` above the page view's bottom edge: the active
//! tool as an accent pill, the three quick tools (Wire, Symbol, Text), a divider and the prompt (the selection and its
//! keys, the pick stack under the pointer, FT-521, or the tool's target and keys). It never takes the keyboard focus
//! and never covers the pointer: while the pointer is within 48 px of it, it stands 12 px under the canvas tab bar.
//! The zoom control is a column of three icon buttons at the bottom right. The HUD replaces the tool strip under the
//! menu bar; the fields of the IP, Text, Wire marker and Structure box tools moved into the "Tool options" card at the
//! page view's top left (`app.rs`).
//!
//! AccessKit: the HUD is one `Status` node named "Tool" whose value is the tool, its target and the prompt; its
//! buttons are named, not focusable (`Sense::CLICK`).

use egui::{Id, Pos2, Rect, Sense, Ui, UiBuilder, Vec2, pos2, vec2};
use ew_viewmodel::command_bar::{CommandButton, action_button, hud};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::Value;

use crate::app::Event;
use crate::command_bar::{PROMPT_OPACITY, Spec, button_response, paint_icon, state_overlay};
use crate::theme::{self, Tokens, TypeToken};

/// How near the pointer may come to the HUD before it moves under the tab bar (doc 16a §5.5), points.
const AVOID: f32 = 48.0;

/// The opacity of the HUD's divider (the mockup's `.hud .div`, a quarter of the text colour).
const DIVIDER_OPACITY: f32 = 0.25;

/// The zoom control's actions, top to bottom (doc 16a §5.5).
const ZOOM: [&str; 3] = ["view.zoom_in", "view.zoom_out", "view.fit_page"];

/// `type.small` in SemiBold: the active tool's name.
fn small_strong(t: &Tokens) -> TypeToken {
    TypeToken {
        semibold: true,
        ..t.type_small
    }
}

/// The bottom band of the page view a fit keeps clear: the HUD's height with `space.5` above and below it.
pub(crate) fn fit_inset(t: &Tokens) -> f32 {
    t.size_control + 2.0 * t.space_2 + 2.0 * t.space_5
}

/// The HUD's rectangle of width `width` in the page view `pane` (doc 16a §5.5): centred, `space.5` above the bottom;
/// `space.4` under the canvas tab bar when `raised`.
pub(crate) fn hud_rect(t: &Tokens, pane: Rect, width: f32, raised: bool) -> Rect {
    let height = t.size_control + 2.0 * t.space_2;
    let top = if raised {
        pane.top() + t.size_dock_tab_bar + t.space_4
    } else {
        pane.bottom() - t.space_5 - height
    };
    Rect::from_min_size(pos2(pane.center().x - width / 2.0, top), vec2(width, height))
}

/// Paints the HUD of `shell` over the page view `pane` and returns its rectangle.
pub(crate) fn show(ctx: &egui::Context, shell: &ShellModel, pane: Rect, events: &mut Vec<Event>) -> Rect {
    let t = theme::current(ctx);
    let h = hud(shell);
    let measure = |text: &str, font: egui::FontId| {
        ctx.fonts_mut(|f| f.layout_no_wrap(text.to_owned(), font, theme::NONE).size())
    };
    let tool_w = t.space_3
        + t.icon_sm
        + t.space_icon
        + measure(&h.tool_label, theme::font(&small_strong(&t))).x
        + t.space_3;
    let prompt_size = measure(&h.prompt, theme::font(&t.type_small));
    let natural = t.space_2
        + tool_w
        + h.quick.len() as f32 * (t.space_1 + t.size_control)
        + 2.0 * t.space_1
        + t.border_width
        + t.space_icon
        + prompt_size.x
        + t.space_4;
    // Cycle 39 (FIX39b-1): the HUD never runs under the zoom column; past its room the prompt wraps onto a second
    // line within the pill's height and only then elides, whole in the HUD's accessible value and in a tooltip on the
    // prompt. The room grows with the page view (no fixed cap).
    let width = natural.min(max_width(&t, pane)).max(natural - prompt_size.x);
    let prompt_room = prompt_size.x - (natural - width);
    let elided = width < natural;
    let lower = hud_rect(&t, pane, width, false);
    let raised = ctx
        .pointer_hover_pos()
        .is_some_and(|p| lower.expand(AVOID).contains(p));
    let rect = hud_rect(&t, pane, width, raised);
    let mut value = h.tool_label.clone();
    if let Some(target) = &h.target {
        value.push(' ');
        value.push_str(target);
    }
    value.push_str(" \u{b7} ");
    value.push_str(&h.prompt);
    egui::Area::new(Id::unique("tool-hud"))
        .fixed_pos(rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            // The shadow stays on the page view: it must not spill onto the bottom dock's tab bar below it.
            ui.set_clip_rect(pane);
            let status_id = Id::unique("tool-hud-status");
            let status = ui.interact(rect, status_id, Sense::hover());
            let name = strings::text("hud-name").to_owned();
            status.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Status, true, &name));
            ui.ctx().accesskit_node_builder(status_id, |n| {
                n.set_value(value.clone());
                n.set_live(egui::accesskit::Live::Polite);
            });
            ui.painter()
                .add(theme::shadow(&t.elevation_2).as_shape(rect, theme::radius(t.radius_pill)));
            ui.painter()
                .rect_filled(rect, theme::radius(t.radius_pill), t.surface_inverse.color32());
            ui.scope_builder(
                UiBuilder::new().max_rect(rect).accessibility_parent(status_id),
                |ui| {
                    let cy = rect.center().y;
                    let mut x = rect.left() + t.space_2;
                    // The active tool.
                    let r =
                        Rect::from_min_size(pos2(x, cy - t.size_control / 2.0), vec2(tool_w, t.size_control));
                    let tool = action_button(shell, h.tool);
                    let response = button_response(
                        ui,
                        Id::unique("hud-tool"),
                        r,
                        Spec {
                            name: &h.tool_label,
                            shortcut: tool.as_ref().map_or("", |b| b.shortcut.as_str()),
                            enabled: true,
                            checked: Some(true),
                            sense: Sense::CLICK,
                        },
                    );
                    ui.painter()
                        .rect_filled(r, theme::radius(t.radius_pill), t.accent.color32());
                    let icon = Rect::from_min_size(
                        pos2(r.left() + t.space_3, cy - t.icon_sm / 2.0),
                        Vec2::splat(t.icon_sm),
                    );
                    paint_icon(ui, h.icon, icon, t.text_on_accent.color32(), t.icon_stroke);
                    let galley = ui.painter().layout_no_wrap(
                        h.tool_label.clone(),
                        theme::font(&small_strong(&t)),
                        t.text_on_accent.color32(),
                    );
                    let at = pos2(icon.right() + t.space_icon, cy - galley.size().y / 2.0);
                    ui.painter().galley(at, galley, t.text_on_accent.color32());
                    if response.clicked() {
                        events.push(Event::Action(h.tool, Value::Null));
                    }
                    x = r.right();
                    // The quick tools.
                    for id in h.quick {
                        x += t.space_1;
                        let r = Rect::from_min_size(
                            pos2(x, cy - t.size_control / 2.0),
                            Vec2::splat(t.size_control),
                        );
                        x = r.right();
                        let Some(b) = action_button(shell, id) else {
                            continue;
                        };
                        quick_button(ui, &t, &b, r, events);
                    }
                    // The divider and the prompt.
                    x += t.space_1;
                    let divider = t.text_on_inverse.color32().gamma_multiply(DIVIDER_OPACITY);
                    ui.painter().line_segment(
                        [pos2(x, cy - t.icon_sm / 2.0), pos2(x, cy + t.icon_sm / 2.0)],
                        egui::Stroke {
                            width: t.border_width,
                            color: divider,
                        },
                    );
                    x += t.border_width + t.space_1 + t.space_icon;
                    let colour = t.text_on_inverse.color32().gamma_multiply(PROMPT_OPACITY);
                    let galley = prompt_galley(ui, &t, &h.prompt, prompt_room, elided, colour);
                    let at = pos2(x, cy - galley.size().y / 2.0);
                    let frame = Rect::from_min_size(at, galley.size());
                    let galley_cut = galley.elided;
                    ui.painter().galley(at, galley, colour);
                    if galley_cut {
                        let response = ui.interact(frame, Id::unique("hud-prompt"), Sense::hover());
                        crate::components::show_tip(
                            ui.ctx(),
                            egui::containers::Tooltip::for_enabled(&response),
                            &h.prompt,
                        );
                    }
                },
            );
        });
    rect
}

/// The widest the HUD may be in the page view `pane`: centred, it keeps clear of the zoom column at the right (and
/// as much at the left), with `space.2` between them (cycle 39, FIX39b-1).
pub(crate) fn max_width(t: &Tokens, pane: Rect) -> f32 {
    (pane.width() - 2.0 * (t.space_4 + t.size_control + t.space_2)).max(0.0)
}

/// The most lines the HUD's prompt takes: two `type.small` lines stand within the pill's height.
const PROMPT_LINES: usize = 2;

/// The HUD's prompt: on one line when it fits, else wrapped at `room` points onto at most [`PROMPT_LINES`] lines,
/// the last ending in "…" when it is still longer.
fn prompt_galley(
    ui: &Ui,
    t: &Tokens,
    prompt: &str,
    room: f32,
    wrap: bool,
    colour: egui::Color32,
) -> std::sync::Arc<egui::Galley> {
    let mut job =
        egui::text::LayoutJob::simple_singleline(prompt.to_owned(), theme::font(&t.type_small), colour);
    if !wrap {
        return ui.fonts_mut(|f| f.layout_job(job));
    }
    job.wrap.max_width = room.max(0.0);
    job.wrap.max_rows = PROMPT_LINES;
    job.wrap.overflow_character = Some('…');
    ui.fonts_mut(|f| f.layout_job(job))
}

/// A quick tool of the HUD: a 28-px icon pill named by its label.
fn quick_button(ui: &Ui, t: &Tokens, b: &CommandButton, r: Rect, events: &mut Vec<Event>) {
    let response = button_response(
        ui,
        Id::unique(("hud-quick", b.id)),
        r,
        Spec {
            name: &b.label,
            shortcut: &b.shortcut,
            enabled: b.enabled,
            checked: None,
            sense: Sense::CLICK,
        },
    );
    state_overlay(ui, t, &response, b.enabled, t.radius_pill);
    let colour = if b.enabled {
        t.text_on_inverse.color32()
    } else {
        t.text_on_inverse.color32().gamma_multiply(PROMPT_OPACITY)
    };
    let icon = Rect::from_center_size(r.center(), Vec2::splat(t.icon_sm));
    paint_icon(ui, b.icon, icon, colour, t.icon_stroke);
    if response.clicked() && b.enabled {
        events.push(Event::Action(b.id, Value::Null));
    }
}

/// The zoom control's rectangle in `pane`: `space.4` from the right, `space.5` from the bottom (doc 16a §5.5).
pub(crate) fn zoom_rect(t: &Tokens, pane: Rect) -> Rect {
    let size = vec2(t.size_control, ZOOM.len() as f32 * t.size_control);
    Rect::from_min_size(
        Pos2::new(
            pane.right() - t.space_4 - size.x,
            pane.bottom() - t.space_5 - size.y,
        ),
        size,
    )
}

/// Paints the zoom control over the page view `pane`: Zoom in, Zoom out and Fit page.
pub(crate) fn show_zoom(ctx: &egui::Context, shell: &ShellModel, pane: Rect, events: &mut Vec<Event>) {
    let t = theme::current(ctx);
    let rect = zoom_rect(&t, pane);
    egui::Area::new(Id::unique("zoom-control"))
        .fixed_pos(rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            // The shadow stays on the page view: it must not spill onto the bottom dock's tab bar below it.
            ui.set_clip_rect(pane);
            ui.painter()
                .add(theme::shadow(&t.elevation_1).as_shape(rect, theme::radius(t.radius_md)));
            ui.painter()
                .rect_filled(rect, theme::radius(t.radius_md), t.surface_raised.color32());
            for (i, id) in ZOOM.iter().enumerate() {
                let Some(b) = action_button(shell, id) else {
                    continue;
                };
                let r = Rect::from_min_size(
                    pos2(rect.left(), rect.top() + i as f32 * t.size_control),
                    Vec2::splat(t.size_control),
                );
                let response = button_response(
                    ui,
                    Id::unique(("zoom-control", b.id)),
                    r,
                    Spec {
                        name: &b.label,
                        shortcut: &b.shortcut,
                        enabled: b.enabled,
                        checked: None,
                        sense: Sense::CLICK,
                    },
                );
                state_overlay(ui, &t, &response, b.enabled, t.radius_md);
                let colour = if b.enabled {
                    t.text_secondary
                } else {
                    t.text_disabled
                };
                let icon = Rect::from_center_size(r.center(), Vec2::splat(t.icon_sm));
                paint_icon(ui, b.icon, icon, colour.color32(), t.icon_stroke);
                if response.clicked() && b.enabled {
                    events.push(Event::Action(b.id, Value::Null));
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use egui::{Rect, pos2};

    use super::{hud_rect, max_width, zoom_rect};
    use crate::theme::{self, Density};

    /// Cycle 39 (FIX39b-1): the widest HUD stays clear of the zoom column at any page view width; the room grows
    /// with the page view (no cap chosen for the floor).
    #[test]
    fn the_widest_hud_keeps_clear_of_the_zoom_column() {
        let t = theme::tokens(theme::Theme::Light, Density::Comfortable);
        for width in [600.0, 1000.0, 1600.0] {
            let pane = Rect::from_min_max(pos2(100.0, 50.0), pos2(100.0 + width, 750.0));
            let hud = hud_rect(&t, pane, max_width(&t, pane), false);
            let zoom = zoom_rect(&t, pane);
            assert!(
                hud.right() + t.space_2 <= zoom.left() + 0.01,
                "{width}: {hud:?} {zoom:?}"
            );
            assert!(hud.left() >= pane.left(), "{width}");
        }
        let narrow = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
        let wide = Rect::from_min_max(pos2(0.0, 0.0), pos2(1600.0, 600.0));
        assert!(
            max_width(&t, wide) - max_width(&t, narrow) >= 799.0,
            "the room grows with the pane"
        );
    }
}
