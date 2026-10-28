// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The toasts and the notifications log of the window (WP-S2.09 part 1, D6, §6.5; FR-UI-042, NFR-A11Y-04).
//!
//! **Toasts.** At most three, stacked at the bottom right above the status bar, newest at the bottom; each is a polite
//! live region with its action (Undo, Open; disabled once it is no longer valid) and Close. Each goes 8 seconds after
//! the frame that first showed it, on the window's clock ([`crate::app::App`] keeps the times); the log keeps it.
//!
//! **The log.** A window of the session's notifications, newest first, with each one's action as a button while it is
//! valid; Escape or Close closes it (`notifications.show`). It never takes the focus by itself.

use egui::{Align2, Context, Ui};
use ew_viewmodel::notifications::Notification;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// One notification's text (a polite live region), its action button and, for a toast, Close.
fn row(ui: &mut Ui, shell: &ShellModel, n: &Notification, close: bool, events: &mut Vec<Event>) {
    ui.horizontal_wrapped(|ui| {
        let response = ui.label(&n.text);
        let live = n.text.clone();
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_live(egui::accesskit::Live::Polite);
            node.set_value(live);
        });
        if let Some(action) = n.action() {
            let valid = shell.notification_valid(n);
            if ui
                .add_enabled(valid, egui::Button::new(strings::text(action.label_key())))
                .clicked()
            {
                events.push(Event::Action(
                    "notifications.run",
                    json!({ "entry": n.serial, "action": action.word() }),
                ));
            }
        }
        if close && ui.button(strings::text("toast-close")).clicked() {
            events.push(Event::DismissToast(n.serial));
        }
    });
}

/// The visible toasts of `shell`, oldest first and the newest at the bottom.
pub(crate) fn show_toasts(ctx: &Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let visible = shell.notifications().visible();
    if visible.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::unique("ew-toasts"))
        .anchor(Align2::RIGHT_BOTTOM, [-12.0, -40.0])
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            for n in visible {
                egui::Frame::popup(ui.style()).show(ui, |ui| row(ui, shell, n, true, events));
            }
        });
}

/// The notifications log while its popup is open.
pub(crate) fn show_log(ctx: &Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let log = shell.notifications();
    if !log.popup_open() {
        return;
    }
    egui::Window::new(strings::text("notifications-title"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::RIGHT_BOTTOM, [-12.0, -40.0])
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            if log.is_empty() {
                ui.label(strings::text("notifications-empty"));
            }
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for n in log.entries() {
                    row(ui, shell, n, false, events);
                }
            });
            if ui.button(strings::text("dialog-close")).clicked() {
                events.push(Event::Action("notifications.show", json!({ "on": false })));
            }
        });
}
