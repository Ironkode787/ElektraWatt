// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The toasts and the notifications log of the window (WP-S2.09 part 1, D6, §6.5; FR-UI-042, NFR-A11Y-04), restyled
//! by WP-S2.17 part 4 (doc 16a §6.10).
//!
//! **Toasts.** At most three, stacked at the canvas's top right, `space.4` from its edges (under the canvas tabs), the
//! newest at the top: 300 px wide (`size.toast`; W40b: [`TOAST_SHARE`] of a wider window), `color.surface.raised`, `elevation.2`, `radius.lg`, `space.4` padding; three
//! columns with `space.3` gaps: the 20-px semantic icon ([`tone_icon`]: a result's worst severity with the icons of
//! §6.24, else a check in `color.success` for a toast with an action, the info icon otherwise), the title in
//! `type.body.strong` with a result's line under it in `type.small` and `color.text.secondary` (the check's
//! "4 errors, 25 warnings, 12 infos" on one line; a refusal's reason wrapped up to [`LINE_ROWS`] rows), and the action as a text link in `type.small` SemiBold
//! `color.accent` (Undo, Open; disabled once it is no longer valid), and a 24-px Dismiss icon button. Each is a polite
//! live region. Without a canvas (the start page) they stand at the window's top right under the command bar. Each
//! goes 8 seconds after the frame that first showed it, on the window's clock ([`crate::app::App`] keeps the times); the
//! log keeps it.
//!
//! **The log.** A popover above the status bar's notifications field, newest first, with each one's action while it is
//! valid; Escape or Close closes it (`notifications.show`). It never takes the focus by itself. Its empty state says
//! so.

use egui::{Align, Align2, Context, Layout, Rect, RichText, Ui, Vec2};
use ew_viewmodel::key;
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

/// The share of the window's width a toast takes once that is wider than `size.toast` (W40b; doc 16a §2: the UI
/// grows with the space it has).
pub(crate) const TOAST_SHARE: f32 = 0.22;

/// The rows a toast's line wraps to before it is cut (W40b: a refusal's reason is never cut at one line; what does
/// not fit in three rows is in the HUD, the log and the tooltip).
pub(crate) const LINE_ROWS: usize = 3;

/// The rows a toast's title wraps to before it is cut (ID-1139: a disabled command's reason is never cut at one line;
/// the whole text is in the toast's tooltip).
pub(crate) const TITLE_ROWS: usize = 2;

/// A toast's width in a window `window` wide: `size.toast`, growing with the window ([`TOAST_SHARE`]).
pub(crate) fn toast_width(t: &Tokens, window: f32) -> f32 {
    crate::dialog::grown(t.size_toast, TOAST_SHARE, window)
}

/// The width of a toast's text column in a toast `width` wide: the card less its padding, the icon, the Dismiss
/// button and the two gaps.
pub(crate) fn text_width(t: &Tokens, width: f32) -> f32 {
    width - 2.0 * t.space_4 - t.icon_md - t.size_control_inline - 2.0 * t.space_3
}

/// The layout of a toast's title or line: wrapped at word boundaries in the text column, up to `rows` rows
/// ([`TITLE_ROWS`], [`LINE_ROWS`]), then cut with an ellipsis.
fn line_job(line: &str, font: egui::FontId, colour: egui::Color32, rows: usize) -> egui::text::LayoutJob {
    let mut job =
        egui::text::LayoutJob::single_section(line.to_owned(), egui::TextFormat::simple(font, colour));
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = false;
    job
}

/// One toast (doc 16a §6.10); returns whether the pointer is over it (hovering pauses it).
fn toast(ui: &mut Ui, t: &Tokens, shell: &ShellModel, n: &Notification, events: &mut Vec<Event>) -> bool {
    let width = toast_width(t, ui.ctx().content_rect().width());
    let frame = egui::Frame::new()
        .fill(t.surface_raised.color32())
        .stroke(theme::stroke(t.border_raised_width, t.border_default))
        .corner_radius(theme::radius(t.radius_lg))
        .shadow(theme::shadow(&t.elevation_2))
        .inner_margin(egui::Margin::same(crate::theme::margin_px(t.space_4)))
        .show(ui, |ui| {
            let inner = width - 2.0 * t.space_4;
            ui.set_width(inner);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = t.space_3;
                let (icon, colour) = tone_icon(t, n.tone());
                let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_md), egui::Sense::hover());
                icons::paint(ui.painter(), icon, r, colour.color32(), t.icon_stroke);
                let text_w = text_width(t, width);
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.spacing_mut().item_spacing.y = t.space_icon;
                    match n.lines() {
                        (title, Some(line)) => {
                            // The title carries the whole text as the live region. It wraps at word boundaries up to two
                            // rows (ID-1139: a disabled command's reason is never cut at one line), the line up to three
                            // (W40b), each then cut with an ellipsis; the whole text is the toast's tooltip (ID-940), so
                            // the labels show no tooltip of their own. A check's counts stay on one line (FT-664: the
                            // waived and pending counts qualify the title, so the severities fit).
                            let response = ui.add(
                                egui::Label::new(line_job(
                                    title,
                                    theme::font(&t.type_body_strong),
                                    t.text_primary.color32(),
                                    TITLE_ROWS,
                                ))
                                .wrap()
                                .show_tooltip_when_elided(false),
                            );
                            let live = n.text.clone();
                            ui.ctx().accesskit_node_builder(response.id, |node| {
                                node.set_live(egui::accesskit::Live::Polite);
                                node.set_value(live);
                            });
                            ui.add(
                                egui::Label::new(line_job(
                                    line,
                                    theme::font(&t.type_small),
                                    t.text_secondary.color32(),
                                    LINE_ROWS,
                                ))
                                .wrap()
                                .show_tooltip_when_elided(false),
                            );
                        }
                        (_, None) => {
                            live_text(ui, n, theme::font(&t.type_body_strong), t.text_primary.color32());
                        }
                    }
                    action_link(ui, t, shell, n, events);
                });
                if components::icon_button(ui, IconId::x, strings::text(key!("toast-dismiss"))).clicked() {
                    events.push(Event::DismissToast(n.serial));
                }
            });
        });
    // A `Group` node over the card, named "Notification" (its text is the live region inside). Hovering it shows the
    // whole text (ID-940, ID-1139), unless a control inside shows its own tooltip.
    let node = ui
        .interact(
            frame.response.rect,
            ui.scope_id().with(("toast", n.serial)),
            egui::Sense::hover(),
        )
        .on_hover_text(n.text.clone());
    node.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text(key!("toast-name")))
    });
    ui.rect_contains_pointer(frame.response.rect)
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

/// A toast's opacity `age` seconds (frame clock) after the frame that first painted it, for a toast that lives `life`
/// seconds and fades over `fade` seconds (`motion.base`): it fades in over the first `fade` seconds and out over the
/// last, linearly, and stands at 1 between (FT-1027, ID-1107; doc 16a §4.12, §6.10). No fade (`motion.none`): 1.
pub(crate) fn toast_opacity(age: f64, life: f64, fade: f64) -> f32 {
    if fade <= 0.0 {
        return 1.0;
    }
    ((age / fade).min((life - age) / fade)).clamp(0.0, 1.0) as f32
}

/// The visible toasts of `shell`, the newest at the top, anchored at `anchor` (their top-right corner), each painted
/// at its opacity in `opacity` (1 when absent); `under_modal` puts them in the middle order, below a modal dialog's
/// backdrop (doc 16a §6.12), so the dialog is the only focus. Returns the serials of the toasts under the pointer.
pub(crate) fn show_toasts(
    ctx: &Context,
    shell: &ShellModel,
    anchor: egui::Pos2,
    under_modal: bool,
    opacity: &std::collections::BTreeMap<u64, f32>,
    events: &mut Vec<Event>,
) -> Vec<u64> {
    let visible = shell.notifications().visible();
    let mut hovered = Vec::new();
    if visible.is_empty() {
        return hovered;
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
        // The fade is the toasts' own, on the frame clock (FT-1027), not the area's.
        .fade_in(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = t.space_3;
            for n in visible.into_iter().rev() {
                let alpha = opacity.get(&n.serial).copied().unwrap_or(1.0);
                let over = ui
                    .scope(|ui| {
                        ui.multiply_opacity(alpha);
                        toast(ui, &t, shell, n, events)
                    })
                    .inner;
                if over {
                    hovered.push(n.serial);
                }
            }
        });
    hovered
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
                .inner_margin(egui::Margin::same(crate::theme::margin_px(t.space_2)))
                .show(ui, |ui| {
                    let width = t.size_toast + t.size_dialog_small / 4.0;
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.add_space(t.space_2);
                        ui.label(
                            RichText::new(strings::text(key!("notifications-title")))
                                .font(theme::font(&t.type_body_strong))
                                .color(t.text_primary.color32()),
                        );
                    });
                    if log.is_empty() {
                        components::empty_state(
                            ui,
                            IconId::bell,
                            strings::text(key!("notifications-empty")),
                            strings::text(key!("empty-notifications-text")),
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
                            strings::text(key!("dialog-close")),
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

    /// W40b: a toast keeps `size.toast` at the floor, is wider at 1920 and stays inside a narrow window.
    #[test]
    fn the_toast_grows_with_the_window() {
        let t = theme::tokens(Theme::Light, Density::Comfortable);
        assert_eq!(toast_width(&t, 1280.0), t.size_toast);
        let typical = toast_width(&t, 1920.0);
        assert!(typical > t.size_toast && typical < 1920.0, "{typical} at 1920");
        assert_eq!(toast_width(&t, 200.0), 200.0);
    }

    /// W40b: a refusal's reason wraps at word boundaries up to three rows, then is cut with an ellipsis.
    #[test]
    fn the_line_wraps_up_to_three_rows() {
        let ctx = egui::Context::default();
        let reason =
            "0V already has a source on page 101. Press D for a target, or mark one source as the star.";
        let mut rows = (0, 0);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let t = theme::tokens(Theme::Light, Density::Comfortable);
            let font = theme::font(&t.type_small);
            let mut job = line_job(reason, font.clone(), theme::NO_TINT, LINE_ROWS);
            job.wrap.max_width = 120.0;
            let short = ui.painter().layout_job(job);
            let mut job = line_job(reason, font, theme::NO_TINT, LINE_ROWS);
            job.wrap.max_width = 260.0;
            let wide = ui.painter().layout_job(job);
            assert!(
                short.elided && !wide.elided,
                "cut only when three rows do not hold it"
            );
            rows = (short.rows.len(), wide.rows.len());
        });
        output.textures_delta.clear();
        assert_eq!(rows.0, LINE_ROWS);
        assert!(rows.1 > 1 && rows.1 <= LINE_ROWS, "{} rows", rows.1);
    }

    /// ID-1139: at the floor, a disabled click's title lays out whole on two rows in the text column, and a longer one
    /// is cut with an ellipsis after two rows. The toast as `toast` draws it is asserted in the window's tests
    /// (`tests/it/part4_cards.rs`, C46-p1#5).
    #[test]
    fn the_title_wraps_up_to_two_rows() {
        let reason = strings::text(key!("reason-page-type-no-symbols-panel"));
        let (title, _) = reason.split_once(": ").expect("a title and a line");
        assert_eq!(title, "A panel layout page takes no symbols or wires");
        let longer = "A panel layout page takes no symbols, no wires, no terminals, no cables and no black boxes of any \
                      kind at all";
        // The window's fonts, as the toast lays out with them (they take effect with the next pass).
        let ctx = egui::Context::default();
        crate::fonts::install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let mut shown = (0, false, 0, false, String::new());
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let t = theme::tokens(Theme::Light, Density::Comfortable);
            let font = theme::font(&t.type_body_strong);
            let column = text_width(&t, toast_width(&t, 1280.0));
            let mut job = line_job(title, font.clone(), theme::NO_TINT, TITLE_ROWS);
            job.wrap.max_width = column;
            let whole = ui.painter().layout_job(job);
            let mut job = line_job(longer, font, theme::NO_TINT, TITLE_ROWS);
            job.wrap.max_width = column;
            let cut = ui.painter().layout_job(job);
            shown = (
                whole.rows.len(),
                whole.elided,
                cut.rows.len(),
                cut.elided,
                whole.text().to_owned(),
            );
        });
        output.textures_delta.clear();
        assert_eq!(shown, (2, false, TITLE_ROWS, true, title.to_owned()));
    }
}
