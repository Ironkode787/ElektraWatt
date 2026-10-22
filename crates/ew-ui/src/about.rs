// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialogs of the shell (WP-S2.01 part 1, spec section 5.3): About (FR-UI-038 part-1 subset: the version, the
//! page-view renderer, "Update check: off. Crash reports: none", the licence and the repository), the keyboard
//! shortcuts, the unsaved-changes question and Go to page. Each answer is an [`Event`]; Escape closes the dialog
//! through `ui.dismiss`.

use egui::{Align2, Id, TextEdit, Ui, Window};
use ew_viewmodel::actions::ACTIONS;
use ew_viewmodel::shell::{ConfirmAnswer, Dialog, ShellModel};
use ew_viewmodel::strings;
use serde_json::Value;

use crate::app::Event;
use crate::keys;

fn window(title_key: &str) -> Window<'static> {
    Window::new(strings::text(title_key))
        .id(Id::unique("dialog"))
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
}

fn close_button(ui: &mut Ui, events: &mut Vec<Event>) {
    if ui.button(strings::text("dialog-close")).clicked() {
        events.push(Event::Action("ui.dismiss", Value::Null));
    }
}

/// Paints the open dialog, if any.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    match shell.dialog() {
        None => {}
        Some(Dialog::About) => {
            window("about-title").show(ctx, |ui| {
                ui.heading(strings::text("start-title"));
                ui.label(strings::format("about-version", &[&shell.config().app_version]));
                ui.label(strings::text("about-page-view"));
                ui.label(strings::text("about-privacy"));
                ui.label(strings::text("about-licence"));
                ui.label(strings::format(
                    "about-repository",
                    &[env!("CARGO_PKG_REPOSITORY")],
                ));
                close_button(ui, events);
            });
        }
        Some(Dialog::Shortcuts) => {
            window("shortcuts-title").show(ctx, |ui| {
                egui::Grid::new("shortcuts").num_columns(2).show(ui, |ui| {
                    for meta in ACTIONS
                        .iter()
                        .filter(|a| !a.headless_only && !a.shortcut.is_empty())
                    {
                        ui.label(strings::text(meta.label_key));
                        let keys: Vec<String> = meta.shortcut.iter().map(|s| keys::display(s)).collect();
                        ui.label(keys.join(", "));
                        ui.end_row();
                    }
                });
                close_button(ui, events);
            });
        }
        Some(Dialog::ConfirmDiscard { .. }) => {
            window("confirm-title").show(ctx, |ui| {
                ui.label(strings::text("confirm-text"));
                ui.horizontal(|ui| {
                    for (key, answer) in [
                        ("dialog-save", ConfirmAnswer::Save),
                        ("dialog-discard", ConfirmAnswer::Discard),
                        ("dialog-cancel", ConfirmAnswer::Cancel),
                    ] {
                        if ui.button(strings::text(key)).clicked() {
                            events.push(Event::Confirm(answer));
                        }
                    }
                });
            });
        }
        Some(Dialog::GotoPage { text }) => {
            window("goto-title").show(ctx, |ui| {
                let mut field = text.clone();
                let edit = ui.add(TextEdit::singleline(&mut field).hint_text(strings::text("goto-hint")));
                if just_opened {
                    edit.request_focus();
                }
                if field != *text {
                    events.push(Event::GotoText(field.clone()));
                }
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(strings::text("dialog-go")).clicked() || enter {
                        events.push(Event::GotoGo(field.clone()));
                    }
                    if ui.button(strings::text("dialog-cancel")).clicked() {
                        events.push(Event::Action("ui.dismiss", Value::Null));
                    }
                });
            });
        }
    }
}
