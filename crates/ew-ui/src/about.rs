// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialogs of the shell (WP-S2.01 parts 1 and 2, spec section 5.3): the complete About (FR-UI-038: the version,
//! the page-view renderer, the privacy state, the licence, the repository, the engineering-verification
//! disclaimer and the support status, with buttons for the third-party notices and the documentation), the
//! notices, the keyboard shortcuts, the unsaved-changes question, the question about an earlier session's journal,
//! the recovery dialog (WP-S2.09 part 1), Go to page, Save layout as and the creation dialogs of pages, structure nodes
//! and external references (FT-408, FT-409). Each answer is an [`Event`]; Escape closes the dialog through
//! `ui.dismiss`.

use egui::{ScrollArea, TextEdit, Ui};
use ew_viewmodel::dialogs::DialogAnswer;
use ew_viewmodel::shell::{Dialog, FormDialog, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

/// The time of epoch milliseconds `ms` as `2026-10-28 14:03` (UTC, the recovery dialog's hover text since FT-503: the standard library has no local time zone and the
/// application takes no new dependency for one); `None` for 0 (unknown).
fn utc_text(ms: u64) -> Option<String> {
    if ms == 0 {
        return None;
    }
    let d = ew_viewmodel::ew_api::output::date_from_epoch(i64::try_from(ms / 1000).ok()?)?;
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.year, d.month, d.day, d.hour, d.minute
    ))
}

/// Now in epoch milliseconds, from the clock the lock and the journal use (`ew_store::SystemClock`); 0 before 1970.
fn now_ms() -> u64 {
    use ew_viewmodel::ew_api::Clock as _;
    ew_viewmodel::ew_api::SystemClock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
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

/// Paints the open dialog, if any, in the dialog frame of WP-S2.17 part 4 ([`crate::dialog`]): the specification's
/// title, body and buttons, with each dialog's own content between the body and the footer.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let Some(d) = shell.dialog() else {
        return;
    };
    let spec = ew_viewmodel::dialogs::spec(d, shell);
    let has_field = matches!(
        d,
        Dialog::GotoPage { .. } | Dialog::SaveWorkspace { .. } | Dialog::Form(_)
    );
    // What Submit sends (the field's text as typed this frame) and whether Enter in a field submitted.
    let mut submit_text: Option<String> = None;
    let mut submit = false;
    let clicked = crate::dialog::show(ctx, &spec, just_opened && !has_field, |ui| match d {
        Dialog::About => {}
        Dialog::Notices => {
            let text = ew_viewmodel::about::notices_text(shell);
            let t = crate::theme::current(ui.ctx());
            ScrollArea::vertical()
                .max_height(t.size_row * 12.0)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text).monospace()).wrap());
                });
        }
        Dialog::Shortcuts => {
            let t = crate::theme::current(ui.ctx());
            ScrollArea::vertical()
                .max_height(t.size_row * 14.0)
                .show(ui, |ui| {
                    egui::Grid::new("shortcuts").num_columns(2).show(ui, |ui| {
                        for (label, shortcut) in ew_viewmodel::dialogs::shortcut_rows() {
                            ui.label(label);
                            ui.horizontal(|ui| {
                                for s in shortcut {
                                    crate::components::key_chip(ui, s);
                                }
                            });
                            ui.end_row();
                        }
                    });
                });
        }
        Dialog::ConfirmPurge { files, .. } => {
            let t = crate::theme::current(ui.ctx());
            for f in files.iter().take(ew_viewmodel::shell::PURGE_LISTED) {
                ui.label(
                    egui::RichText::new(f)
                        .font(crate::theme::font(&t.type_small))
                        .color(t.text_secondary.color32()),
                );
            }
        }
        // FT-503: the elapsed time against the store's clock; the exact time (UTC, no time-zone source) as a small
        // secondary line under it (W31-5: a hover tooltip covered the buttons).
        Dialog::Recovery {
            last_change_ms,
            failed,
            ..
        } => {
            let t = crate::theme::current(ui.ctx());
            if let Some(when) = utc_text(*last_change_ms) {
                let ago = strings::elapsed_text(*last_change_ms, now_ms());
                ui.label(strings::format("recovery-last-change", &[&ago]));
                ui.label(
                    egui::RichText::new(strings::format("recovery-last-change-utc", &[&when]))
                        .font(crate::theme::font(&t.type_small))
                        .color(t.text_tertiary.color32()),
                );
            }
            if let Some(reason) = failed {
                ui.colored_label(t.error.color32(), strings::format("recovery-failed", &[reason]));
            }
        }
        Dialog::GotoPage { text } => {
            let (changed, enter) = name_field(ui, "goto-hint", text, just_opened);
            if let Some(t) = &changed {
                events.push(Event::GotoText(t.clone()));
            }
            submit_text = Some(changed.unwrap_or_else(|| text.clone()));
            submit |= enter;
        }
        Dialog::SaveWorkspace { text } => {
            let (changed, enter) = name_field(ui, "save-workspace-hint", text, just_opened);
            if let Some(t) = &changed {
                events.push(Event::WorkspaceText(t.clone()));
            }
            submit_text = Some(changed.unwrap_or_else(|| text.clone()));
            submit |= enter;
        }
        Dialog::Form(form) => {
            submit |= form_fields(ui, form, just_opened, events);
        }
        Dialog::ConfirmDiscard { .. } | Dialog::ConfirmCheckScheme { .. } | Dialog::ConfirmJournalDiscard => {
        }
    });
    if clicked == Some(DialogAnswer::Submit) {
        submit = true;
    }
    match clicked {
        Some(DialogAnswer::Action(action, params)) => events.push(Event::Action(action, params)),
        Some(DialogAnswer::Confirm(answer)) => events.push(Event::Confirm(answer)),
        Some(DialogAnswer::Dismiss) => events.push(Event::Action("ui.dismiss", Value::Null)),
        Some(DialogAnswer::OpenPrivacy) => events.push(Event::OpenPrivacy),
        Some(DialogAnswer::Submit) | None => {}
    }
    if submit {
        match d {
            Dialog::GotoPage { .. } => events.push(Event::GotoGo(submit_text.unwrap_or_default())),
            Dialog::SaveWorkspace { .. } => events.push(Event::Action(
                "view.workspace_save",
                json!({ "name": submit_text.unwrap_or_default() }),
            )),
            Dialog::Form(_) => events.push(Event::FormSubmit),
            _ => {}
        }
    }
}

/// The fields of a creation or renumber form, two columns; `true` when Enter in a text field submits it.
fn form_fields(ui: &mut Ui, form: &FormDialog, just_opened: bool, events: &mut Vec<Event>) -> bool {
    let t = crate::theme::current(ui.ctx());
    let mut submit = false;
    let mut first = just_opened;
    let field_width = t.size_dialog_form - 2.0 * t.space_5 - t.size_property_label * 2.0;
    egui::Grid::new("form").num_columns(2).show(ui, |ui| {
        for f in &form.fields {
            let label_text = strings::text(f.label_key);
            let label = ui.label(label_text);
            if f.choices.is_empty() {
                let mut text = f.value.clone();
                let edit = ui
                    .add(
                        TextEdit::singleline(&mut text)
                            .hint_text(label_text)
                            .desired_width(field_width),
                    )
                    .labelled_by(label.id);
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
                    .width(field_width)
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
        ui.colored_label(t.error.color32(), error);
    }
    submit
}
