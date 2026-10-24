// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialogs of the shell (WP-S2.01 parts 1 and 2, spec section 5.3): the complete About (FR-UI-038: the version,
//! the page-view renderer, the privacy state, the licence, the repository, the engineering-verification
//! disclaimer and the support status, with buttons for the third-party notices and the documentation), the
//! notices, the keyboard shortcuts, the unsaved-changes question, the question about an earlier session's journal,
//! Go to page, Save layout as and the creation dialogs of pages, structure nodes and external references (FT-408,
//! FT-409). Each answer is an [`Event`]; Escape closes the dialog through `ui.dismiss`.

use egui::{Align2, Id, ScrollArea, TextEdit, Ui, Window};
use ew_viewmodel::actions::ACTIONS;
use ew_viewmodel::shell::{ConfirmAnswer, Dialog, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

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

/// A one-line text field that tells the shell what is typed (`on_change`) and says "go" on Enter.
fn name_field(ui: &mut Ui, hint_key: &str, text: &str, just_opened: bool) -> (Option<String>, bool) {
    let mut field = text.to_owned();
    let edit = ui.add(TextEdit::singleline(&mut field).hint_text(strings::text(hint_key)));
    if just_opened {
        edit.request_focus();
    }
    let changed = (field != text).then(|| field.clone());
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    (changed, enter)
}

/// Paints the open dialog, if any.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    match shell.dialog() {
        None => {}
        Some(Dialog::About) => {
            window("about-title").show(ctx, |ui| {
                ui.heading(strings::text("start-title"));
                ui.set_max_width(460.0);
                for line in ew_viewmodel::about::about_lines(shell) {
                    ui.label(line);
                }
                ui.horizontal(|ui| {
                    if ui.button(strings::text("about-privacy-button")).clicked() {
                        events.push(Event::OpenPrivacy);
                    }
                    if ui.button(strings::text("about-notices-button")).clicked() {
                        events.push(Event::Action("help.notices", Value::Null));
                    }
                    if ui.button(strings::text("about-docs-button")).clicked() {
                        events.push(Event::Action("help.documentation", json!({ "topic": "about" })));
                    }
                    close_button(ui, events);
                });
            });
        }
        Some(Dialog::Notices) => {
            window("notices-title").show(ctx, |ui| {
                let text = ew_viewmodel::about::notices_text(shell);
                ScrollArea::vertical()
                    .max_height(360.0)
                    .max_width(640.0)
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap());
                    });
                close_button(ui, events);
            });
        }
        Some(Dialog::Shortcuts) => {
            window("shortcuts-title").show(ctx, |ui| {
                egui::Grid::new("shortcuts").num_columns(2).show(ui, |ui| {
                    for meta in ACTIONS
                        .iter()
                        .filter(|a| !a.headless_only && !a.hidden && !a.shortcut.is_empty())
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
        Some(Dialog::ConfirmCheckScheme { scheme }) => {
            window("confirm-check-scheme-title").show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(strings::format("confirm-check-scheme-text", &[scheme]));
                ui.horizontal(|ui| {
                    if ui.button(strings::text("dialog-apply")).clicked() {
                        events.push(Event::Action(
                            "wizard.apply_check_scheme",
                            serde_json::json!({ "confirm": true }),
                        ));
                    }
                    if ui.button(strings::text("dialog-cancel")).clicked() {
                        events.push(Event::Action("ui.dismiss", Value::Null));
                    }
                });
            });
        }
        Some(Dialog::ConfirmJournalDiscard) => {
            window("confirm-journal-title").show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(strings::text("confirm-journal-text"));
                ui.horizontal(|ui| {
                    for (key, answer) in [
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
                let (changed, enter) = name_field(ui, "goto-hint", text, just_opened);
                if let Some(t) = &changed {
                    events.push(Event::GotoText(t.clone()));
                }
                let current = changed.unwrap_or_else(|| text.clone());
                ui.horizontal(|ui| {
                    if ui.button(strings::text("dialog-go")).clicked() || enter {
                        events.push(Event::GotoGo(current.clone()));
                    }
                    if ui.button(strings::text("dialog-cancel")).clicked() {
                        events.push(Event::Action("ui.dismiss", Value::Null));
                    }
                });
            });
        }
        Some(Dialog::Form(form)) => {
            window(form.title_key).show(ctx, |ui| {
                let mut submit = false;
                let mut first = just_opened;
                egui::Grid::new("form").num_columns(2).show(ui, |ui| {
                    for f in &form.fields {
                        let label_text = strings::text(f.label_key);
                        let label = ui.label(label_text);
                        if f.choices.is_empty() {
                            let mut text = f.value.clone();
                            let edit = ui.add(
                                TextEdit::singleline(&mut text)
                                    .hint_text(label_text)
                                    .desired_width(240.0),
                            );
                            if first {
                                edit.request_focus();
                                first = false;
                            }
                            if text != f.value {
                                events.push(Event::FormField(f.name, text));
                            }
                            submit |= edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        } else {
                            let combo = egui::ComboBox::from_id_salt(("form-choice", f.name))
                                .selected_text(&f.value)
                                .width(240.0)
                                .show_ui(ui, |ui| {
                                    for choice in f.choices {
                                        if ui.selectable_label(f.value == *choice, *choice).clicked() {
                                            events.push(Event::FormField(f.name, (*choice).to_owned()));
                                        }
                                    }
                                });
                            combo.response.labelled_by(label.id);
                        }
                        ui.end_row();
                    }
                });
                if let Some(error) = &form.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    if ui.button(strings::text("dialog-create")).clicked() {
                        submit = true;
                    }
                    if ui.button(strings::text("dialog-cancel")).clicked() {
                        events.push(Event::Action("ui.dismiss", Value::Null));
                    }
                });
                if submit {
                    events.push(Event::FormSubmit);
                }
            });
        }
        Some(Dialog::SaveWorkspace { text }) => {
            window("save-workspace-title").show(ctx, |ui| {
                let (changed, enter) = name_field(ui, "save-workspace-hint", text, just_opened);
                if let Some(t) = &changed {
                    events.push(Event::WorkspaceText(t.clone()));
                }
                let current = changed.unwrap_or_else(|| text.clone());
                ui.horizontal(|ui| {
                    if ui.button(strings::text("dialog-save-workspace")).clicked() || enter {
                        events.push(Event::Action("view.workspace_save", json!({ "name": current })));
                    }
                    if ui.button(strings::text("dialog-cancel")).clicked() {
                        events.push(Event::Action("ui.dismiss", Value::Null));
                    }
                });
            });
        }
    }
}
