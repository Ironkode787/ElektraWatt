// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool HUD and the zoom control over the canvas (doc 16a §5.5), from
//! `ew_viewmodel::command_bar::hud`. The HUD is a pill centred `space.5` above the page view's bottom edge: the active
//! tool as an accent pill, the three quick tools (Wire, Symbol, Text), a divider and the prompt (the selection and its
//! keys, the pick stack under the pointer, FT-521, or the tool's target and keys). It never takes the keyboard focus
//! and never covers the pointer: while the pointer is within 48 px of it, it stands 12 px under the canvas tab bar, and
//! goes back once the pointer is more than 96 px away (ID-1497 (11)); it keeps clear of the toast stack.
//! The zoom control is a column of three icon buttons at the bottom right. The HUD replaces the tool strip under the
//! menu bar; the fields of the IP, Text, Wire marker and Structure box tools moved into the "Tool options" card at the
//! page view's top left (`app.rs`).
//!
//! AccessKit: the HUD is one `Status` node named "Tool" whose value is the tool, its target and the prompt; its
//! buttons are named, not focusable (`Sense::CLICK`).

use egui::{Id, Pos2, Rect, Sense, Ui, UiBuilder, Vec2, pos2, vec2};
use ew_viewmodel::command_bar::{CommandButton, action_button, hud};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::Value;

use crate::app::Event;
use crate::command_bar::{PROMPT_OPACITY, Spec, button_response, paint_icon, state_overlay};
use crate::theme::{self, Tokens, TypeToken};

/// How near the pointer may come to the HUD before it moves under the tab bar (doc 16a §5.5), points.
const AVOID: f32 = 48.0;

/// How far the pointer must go from the HUD's place at the bottom before it moves back there (ID-1497 (11): with
/// the gap between 48 and 96 px it does not flip back and forth), points.
const RETURN: f32 = 96.0;

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

/// The place of a HUD at `rect` beside the toast stack `toasts` (ID-1497 (11)): where they meet, the HUD sits above
/// the stack's top, or under its bottom where the page view `pane` has no room above it.
pub(crate) fn clear_of_toasts(t: &Tokens, pane: Rect, rect: Rect, toasts: Option<Rect>) -> Rect {
    let Some(stack) = toasts.filter(|s| s.intersects(rect)) else {
        return rect;
    };
    let above = stack.top() - t.space_4 - rect.height();
    let top = if above >= pane.top() + t.size_dock_tab_bar {
        above
    } else {
        stack.bottom() + t.space_4
    };
    rect.translate(vec2(0.0, top - rect.top()))
}

/// Paints the HUD of `shell` over the page view `pane`, clear of the last frame's toast stack `toasts`, and returns
/// its rectangle.
pub(crate) fn show(
    ctx: &egui::Context,
    shell: &ShellModel,
    pane: Rect,
    toasts: Option<Rect>,
    events: &mut Vec<Event>,
) -> Rect {
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
    // A wrapped prompt takes the narrowest room that keeps its two rows, and the pill narrows with it:
    // egui's wrap is greedy, so at the widest room the second row can be a single word.
    let (width, prompt_room) = if elided {
        let room = balanced_room(ctx, &t, &h.prompt, prompt_room);
        (width - (prompt_room - room), room)
    } else {
        (width, prompt_room)
    };
    let lower = hud_rect(&t, pane, width, false);
    // Raised while the pointer is within `AVOID` of the HUD's place at the bottom, back only once it is
    // more than `RETURN` away.
    let raised_id = Id::unique("tool-hud-raised");
    let reach = if ctx.data(|d| d.get_temp::<bool>(raised_id)).unwrap_or(false) {
        RETURN
    } else {
        AVOID
    };
    let raised = ctx
        .pointer_hover_pos()
        .is_some_and(|p| lower.expand(reach).contains(p));
    ctx.data_mut(|d| d.insert_temp(raised_id, raised));
    let rect = clear_of_toasts(&t, pane, hud_rect(&t, pane, width, raised), toasts);
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
            let name = strings::text(key!("hud-name")).to_owned();
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
/// as much at the left), with `space.2` between them (FIX39b-1).
pub(crate) fn max_width(t: &Tokens, pane: Rect) -> f32 {
    (pane.width() - 2.0 * (t.space_4 + t.size_control + t.space_2)).max(0.0)
}

/// The most lines the HUD's prompt takes: two `type.small` lines stand within the pill's height.
const PROMPT_LINES: usize = 2;

/// The rows of `prompt` in `type.small` wrapped at `room` points, with no bound on their number.
fn prompt_rows(ctx: &egui::Context, t: &Tokens, prompt: &str, room: f32) -> usize {
    let mut job =
        egui::text::LayoutJob::simple_singleline(prompt.to_owned(), theme::font(&t.type_small), theme::NONE);
    job.wrap.max_width = room.max(0.0);
    ctx.fonts_mut(|f| f.layout_job(job)).rows.len()
}

/// The narrowest room, at most `room`, at which `prompt` still takes no more rows than at `room`: the rows
/// balance instead of leaving a last row of one word. A prompt longer than [`PROMPT_LINES`] rows at `room` keeps
/// `room` (it elides there). A bisection of a bounded number of steps.
pub(crate) fn balanced_room(ctx: &egui::Context, t: &Tokens, prompt: &str, room: f32) -> f32 {
    let rows = prompt_rows(ctx, t, prompt, room);
    if rows > PROMPT_LINES || rows < 2 {
        return room;
    }
    // The rows cannot fit below half the room each.
    let (mut lo, mut hi) = (room / rows as f32, room);
    for _ in 0..16 {
        if hi - lo < 0.5 {
            break;
        }
        let mid = (lo + hi) / 2.0;
        if prompt_rows(ctx, t, prompt, mid) <= rows {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi.ceil().min(room)
}

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

    use super::{balanced_room, clear_of_toasts, hud_rect, max_width, zoom_rect};
    use crate::theme::{self, Density, Tokens};

    /// The HUD keeps clear of the toast stack: where they meet it sits above the stack's top, or under its
    /// bottom where the page view has no room above it; apart from the stack it keeps its place.
    #[test]
    fn the_hud_keeps_clear_of_the_toast_stack() {
        let t = theme::tokens(theme::Theme::Light, Density::Comfortable);
        let pane = Rect::from_min_max(pos2(300.0, 140.0), pos2(1500.0, 900.0));
        let low = hud_rect(&t, pane, 600.0, false);
        let high = hud_rect(&t, pane, 600.0, true);
        assert_eq!(clear_of_toasts(&t, pane, low, None), low);
        // A stack beside it: no move.
        let aside = Rect::from_min_max(pos2(1450.0, 200.0), pos2(1490.0, 400.0));
        assert_eq!(clear_of_toasts(&t, pane, high, Some(aside)), high);
        // A stack at the bottom right that the low HUD meets: the HUD sits above its top.
        let stack = Rect::from_min_max(pos2(700.0, 700.0), pos2(1490.0, 860.0));
        let moved = clear_of_toasts(&t, pane, low, Some(stack));
        assert_eq!(moved.bottom(), stack.top() - t.space_4);
        assert_eq!((moved.left(), moved.width()), (low.left(), low.width()));
        // A stack at the top right that the raised HUD meets, with no room above it: under its bottom.
        let top = Rect::from_min_max(pos2(700.0, high.top()), pos2(1490.0, high.top() + 120.0));
        let under = clear_of_toasts(&t, pane, high, Some(top));
        assert_eq!(under.top(), top.bottom() + t.space_4);
    }

    /// The rows of `prompt` in `type.small` wrapped at `room`, as text.
    fn rows(ctx: &egui::Context, t: &Tokens, prompt: &str, room: f32) -> Vec<String> {
        let mut job = egui::text::LayoutJob::simple_singleline(
            prompt.to_owned(),
            theme::font(&t.type_small),
            theme::NONE,
        );
        job.wrap.max_width = room;
        let galley = ctx.fonts_mut(|f| f.layout_job(job));
        galley
            .rows
            .iter()
            .map(|r| {
                r.glyphs
                    .iter()
                    .map(|g| g.chr)
                    .collect::<String>()
                    .trim()
                    .to_owned()
            })
            .collect()
    }

    /// At the room where egui's greedy wrap leaves one word on the second row, the HUD's prompt takes
    /// a narrower room that keeps two rows, neither a single word; a prompt on one row keeps its room.
    #[test]
    fn a_wrapped_prompt_balances_its_two_rows() {
        let t = theme::tokens(theme::Theme::Light, Density::Comfortable);
        let ctx = egui::Context::default();
        let prompt = "Not placed: 0V already has a source on page 102; press D for a target, or mark one source as the star";
        let mut seen = None;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            let width = |s: &str| {
                ctx.fonts_mut(|f| {
                    f.layout_no_wrap(s.to_owned(), theme::font(&t.type_small), theme::NONE)
                        .size()
                        .x
                })
            };
            // The widest room at which egui's greedy wrap takes two rows: the second is the last word alone.
            let mut room = width(prompt).ceil();
            while rows(ctx, &t, prompt, room).len() < 2 {
                room -= 1.0;
            }
            let greedy = rows(ctx, &t, prompt, room);
            let balanced = balanced_room(ctx, &t, prompt, room);
            let wide = width(prompt) + 10.0;
            seen = Some((
                greedy,
                room,
                balanced,
                rows(ctx, &t, prompt, balanced),
                balanced_room(ctx, &t, prompt, wide) == wide,
            ));
        });
        // The font atlas the pass built is never uploaded here.
        out.textures_delta.clear();
        let (greedy, room, balanced, after, one_row_kept) = seen.expect("a pass ran");
        assert_eq!(greedy.len(), 2, "{greedy:?}");
        assert_eq!(greedy[1], "star", "egui's greedy wrap: {greedy:?}");
        assert!(balanced < room, "{balanced} < {room}");
        assert_eq!(after.len(), 2, "{after:?}");
        assert!(
            after.iter().all(|r| r.split_whitespace().count() > 1),
            "no row is a single word: {after:?}"
        );
        assert!(one_row_kept);
    }

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
