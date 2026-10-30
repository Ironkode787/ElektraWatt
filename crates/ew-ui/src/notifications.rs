// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The toasts and the notifications log of the window (WP-S2.09 part 1, D6, §6.5; FR-UI-042, NFR-A11Y-04), restyled
//! by WP-S2.17 part 4 (doc 16a §6.10).
//!
//! **Toasts.** At most three, stacked at the canvas's top right, `space.4` from its edges (under the canvas tabs), the
//! newest at the top: 300 px wide, `color.surface.raised`, `elevation.2`, `radius.lg`, `space.4` padding; three
//! columns with `space.3` gaps: the 20-px semantic icon ([`tone_icon`]: a result's worst severity with the icons of
//! §6.24, else a check in `color.success` for a toast with an action, the info icon otherwise), the title in
//! `type.body.strong` with a result's line of counts under it in `type.small` and `color.text.secondary` (the check's
//! "4 errors, 25 warnings, 12 infos" on one line), and the action as a text link in `type.small` SemiBold
//! `color.accent` (Undo, Open; disabled once it is no longer valid), and a 24-px Dismiss icon button. Each is a polite
//! live region. Without a canvas (the start page) they stand at the window's top right under the command bar. Each
//! goes 8 seconds after the frame that first showed it, on the window's clock ([`crate::app::App`] keeps the times); the
//! log keeps it.
//!
//! **The log.** A popover above the status bar's notifications field, newest first, with each one's action while it is
//! valid; Escape or Close closes it (`notifications.show`). It never takes the focus by itself. Its empty state says
//! so.

use egui::{Align, Align2, Context, Layout, Rect, RichText, Ui, Vec2};
use ew_viewmodel::notifications::{NoteTone, Notification};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::icons::{self, IconId};
use crate::theme::{self, Tokens};

/// The action of `n` as a text link in `type.small` SemiBold `color.accent`, disabled while it is not valid.
fn action_link(ui: &mut Ui, t: &Tokens, shell: &ShellModel, n: &Notification, events: &mut Vec<Event>) {
    let Some(action) = n.action() else {
        return;
    };
    let valid = shell.notification_valid(n);
    let colour = if valid { t.accent } else { t.text_disabled };
    let font = theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    let link = egui::Button::new(
        RichText::new(strings::text(action.label_key()))
            .font(font)
            .color(colour.color32()),
    )
    .frame(false)
    .min_size(Vec2::ZERO);
    if ui.add_enabled(valid, link).clicked() {
        events.push(Event::Action(
            "notifications.run",
            json!({ "entry": n.serial, "action": action.word() }),
        ));
    }
}

/// The text of `n` as a polite live region in `font`.
fn live_text(ui: &mut Ui, n: &Notification, font: egui::FontId, colour: egui::Color32) {
    let response = ui.add(egui::Label::new(RichText::new(&n.text).font(font).color(colour)).wrap());
    let live = n.text.clone();
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_live(egui::accesskit::Live::Polite);
        node.set_value(live);
    });
}

/// The icon of a toast's tone and its colour (doc 16a §6.10, the severity icons of §6.24): error `circle-x` in
/// `color.error`, warning `alert-triangle` in the warning icon colour, info `info-circle` in `color.info`, success a
/// check in `color.success`.
pub(crate) fn tone_icon(t: &Tokens, tone: NoteTone) -> (IconId, theme::Rgba) {
    match tone {
        NoteTone::Error => (IconId::circle_x, t.error),
        NoteTone::Warning => (IconId::alert_triangle, t.warning_icon),
        NoteTone::Info => (IconId::info_circle, t.info),
        NoteTone::Success => (IconId::circle_check, t.success),
    }
}

/// One toast (doc 16a §6.10).
fn toast(ui: &mut Ui, t: &Tokens, shell: &ShellModel, n: &Notification, events: &mut Vec<Event>) {
    let frame = egui::Frame::new()
        .fill(t.surface_raised.color32())
        .stroke(theme::stroke(t.border_raised_width, t.border_default))
        .corner_radius(theme::radius(t.radius_lg))
        .shadow(theme::shadow(&t.elevation_2))
        .inner_margin(egui::Margin::same(t.space_4 as i8))
        .show(ui, |ui| {
            let inner = t.size_toast - 2.0 * t.space_4;
            ui.set_width(inner);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = t.space_3;
                let (icon, colour) = tone_icon(t, n.tone());
                let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_md), egui::Sense::hover());
                icons::paint(ui.painter(), icon, r, colour.color32(), t.icon_stroke);
                let text_w = inner - t.icon_md - t.size_control_inline - 2.0 * t.space_3;
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.spacing_mut().item_spacing.y = t.space_icon;
                    match n.lines() {
                        (title, Some(line)) => {
                            // The title carries the whole text as the live region; the counts stay on one line
                            // (FT-664: the waived and pending counts qualify the title, so the severities fit; a
                            // longer line is cut with an ellipsis, whole in its tooltip, never wrapped).
                            let response = ui.add(
                                egui::Label::new(
                                    RichText::new(title)
                                        .font(theme::font(&t.type_body_strong))
                                        .color(t.text_primary.color32()),
                                )
                                .truncate(),
                            );
                            let live = n.text.clone();
                            ui.ctx().accesskit_node_builder(response.id, |node| {
                                node.set_live(egui::accesskit::Live::Polite);
                                node.set_value(live);
                            });
                            ui.add(
                                egui::Label::new(
                                    RichText::new(line)
                                        .font(theme::font(&t.type_small))
                                        .color(t.text_secondary.color32()),
                                )
                                .truncate(),
                            );
                        }
                        (_, None) => {
                            live_text(ui, n, theme::font(&t.type_body_strong), t.text_primary.color32());
                        }
                    }
                    action_link(ui, t, shell, n, events);
                });
                if components::icon_button(ui, IconId::x, strings::text("toast-dismiss")).clicked() {
                    events.push(Event::DismissToast(n.serial));
                }
            });
        });
    // A `Group` node over the card, named "Notification" (its text is the live region inside).
    let node = ui.interact(
        frame.response.rect,
        ui.scope_id().with(("toast", n.serial)),
        egui::Sense::hover(),
    );
    node.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("toast-name")));
}

/// Where the toasts stand: the canvas's top right under its tabs, else the window's under the command bar.
pub(crate) fn toast_anchor(t: &Tokens, ctx: &Context, canvas: Option<Rect>, bar_bottom: f32) -> egui::Pos2 {
    match canvas {
        Some(pane) => egui::pos2(
            pane.right() - t.space_4,
            pane.top() + t.size_dock_tab_bar + t.space_4,
        ),
        None => egui::pos2(ctx.content_rect().right() - t.space_4, bar_bottom + t.space_4),
    }
}

/// The id of the toasts' area (tests: its layer).
pub const TOASTS_AREA: &str = "ew-toasts";

/// The visible toasts of `shell`, the newest at the top, anchored at `anchor` (their top-right corner); `under_modal`
/// puts them in the middle order, below a modal dialog's backdrop (doc 16a §6.12), so the dialog is the only focus.
pub(crate) fn show_toasts(
    ctx: &Context,
    shell: &ShellModel,
    anchor: egui::Pos2,
    under_modal: bool,
    events: &mut Vec<Event>,
) {
    let visible = shell.notifications().visible();
    if visible.is_empty() {
        return;
    }
    let t = theme::current(ctx);
    let order = if under_modal {
        egui::Order::Middle
    } else {
        egui::Order::Foreground
    };
    egui::Area::new(egui::Id::unique(TOASTS_AREA))
        .pivot(Align2::RIGHT_TOP)
        .fixed_pos(anchor)
        .order(order)
        .interactable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = t.space_3;
            for n in visible.into_iter().rev() {
                toast(ui, &t, shell, n, events);
            }
        });
}

/// The notifications log while its popover is open.
pub(crate) fn show_log(ctx: &Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let log = shell.notifications();
    if !log.popup_open() {
        return;
    }
    let t = theme::current(ctx);
    egui::Area::new(egui::Id::unique("notifications-log"))
        .anchor(Align2::RIGHT_BOTTOM, [-t.space_4, -(t.size_status + t.space_2)])
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(t.surface_raised.color32())
                .stroke(theme::stroke(t.border_raised_width, t.border_default))
                .corner_radius(theme::radius(t.radius_md))
                .shadow(theme::shadow(&t.elevation_1))
                .inner_margin(egui::Margin::same(t.space_2 as i8))
                .show(ui, |ui| {
                    let width = t.size_toast + t.size_dialog_small / 4.0;
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.add_space(t.space_2);
                        ui.label(
                            RichText::new(strings::text("notifications-title"))
                                .font(theme::font(&t.type_body_strong))
                                .color(t.text_primary.color32()),
                        );
                    });
                    if log.is_empty() {
                        components::empty_state(
                            ui,
                            IconId::bell,
                            strings::text("notifications-empty"),
                            strings::text("empty-notifications-text"),
                            None,
                        );
                    }
                    egui::ScrollArea::vertical()
                        .max_height(t.size_row * 12.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for n in log.entries() {
                                ui.horizontal(|ui| {
                                    ui.set_min_height(t.size_row);
                                    ui.add_space(t.space_2);
                                    ui.vertical(|ui| {
                                        ui.set_width(width - 2.0 * t.space_4);
                                        live_text(ui, n, theme::font(&t.type_body), t.text_primary.color32());
                                        action_link(ui, &t, shell, n, events);
                                    });
                                });
                            }
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if components::button(
                            ui,
                            components::Kind::Subtle,
                            strings::text("dialog-close"),
                            true,
                            0.0,
                        )
                        .clicked()
                        {
                            events.push(Event::Action("notifications.show", json!({ "on": false })));
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Density, Theme};

    #[test]
    fn the_toast_icon_follows_the_tone() {
        let t = theme::tokens(Theme::Light, Density::Comfortable);
        assert_eq!(tone_icon(&t, NoteTone::Error), (IconId::circle_x, t.error));
        assert_eq!(
            tone_icon(&t, NoteTone::Warning),
            (IconId::alert_triangle, t.warning_icon)
        );
        assert_eq!(tone_icon(&t, NoteTone::Info), (IconId::info_circle, t.info));
        assert_eq!(
            tone_icon(&t, NoteTone::Success),
            (IconId::circle_check, t.success)
        );
    }
}
