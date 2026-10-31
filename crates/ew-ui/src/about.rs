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

/// The key of the test-only clock of [`set_now_for_tests`].
const NOW_FOR_TESTS: &str = "ew-now-for-tests";

/// Makes the recovery dialog measure its elapsed time against `now_ms` (epoch milliseconds) instead of the system
/// clock, for a stable image in the pixel gate (FT-571). Tests only: the product never calls it and keeps the store's
/// clock.
#[doc(hidden)]
pub fn set_now_for_tests(ctx: &egui::Context, now_ms: u64) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(NOW_FOR_TESTS), now_ms));
}

/// Now for the recovery dialog: the test clock of [`set_now_for_tests`] when one is set, else [`now_ms`].
fn dialog_now_ms(ctx: &egui::Context) -> u64 {
    ctx.data(|d| d.get_temp::<u64>(egui::Id::unique(NOW_FOR_TESTS)))
        .unwrap_or_else(now_ms)
}

/// A one-line text field that tells the shell what is typed (`on_change`) and says "go" on Enter.
fn name_field(ui: &mut Ui, hint_key: &str, text: &str, just_opened: bool) -> (Option<String>, bool) {
    let t = crate::theme::current(ui.ctx());
    let mut field = text.to_owned();
    // FT-566: a dialog's only field fills the body's width, `size.control` high (doc 16a §6.5).
    let edit = ui.add(
        TextEdit::singleline(&mut field)
            .hint_text(strings::text(hint_key))
            .desired_width(f32::INFINITY)
            .min_size(egui::vec2(0.0, t.size_control)),
    );
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
    let mut spec = ew_viewmodel::dialogs::spec(d, shell);
    // About (doc 16a §6.12, small dialog): the footer keeps the Primary (Close) only; the privacy statement, the
    // notices and the documentation are links in the body, so the footer fits the 440-px frame and shares its left
    // edge with the title and the body.
    let links = if matches!(d, Dialog::About) {
        std::mem::take(&mut spec.secondary)
    } else {
        Vec::new()
    };
    let mut link_clicked: Option<DialogAnswer> = None;
    let has_field = matches!(
        d,
        Dialog::GotoPage { .. }
            | Dialog::SaveWorkspace { .. }
            | Dialog::Form(_)
            | Dialog::PartChoose(_)
            | Dialog::PartEdit(_)
            | Dialog::Shortcuts
    );
    // WP-S2.07 part 4a: Choose picks the list's active row, which the window keeps.
    if matches!(d, Dialog::PartChoose(_)) {
        spec.primary.answer = DialogAnswer::Action(
            "part.pick",
            json!({ "index": crate::part_dialog::active(ctx) + 1 }),
        );
    }
    // What Submit sends (the field's text as typed this frame) and whether Enter in a field submitted.
    let mut submit_text: Option<String> = None;
    let mut submit = false;
    let clicked = crate::dialog::show(ctx, &spec, just_opened && !has_field, |ui| match d {
        Dialog::About => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = crate::theme::current(ui.ctx()).space_4;
                for b in &links {
                    if ui.link(&b.label).clicked() {
                        link_clicked = Some(b.answer.clone());
                    }
                }
            });
        }
        Dialog::Notices => {
            let blocks = notice_blocks(&ew_viewmodel::about::notices_text(shell));
            let t = crate::theme::current(ui.ctx());
            ScrollArea::vertical()
                .max_height(t.size_row * 12.0)
                .show(ui, |ui| {
                    // FT-559: the underlined headings in `type.body.strong`, the text between them reflowed.
                    for (i, block) in blocks.iter().enumerate() {
                        match block {
                            NoticeBlock::Heading(h) => {
                                if i > 0 {
                                    ui.add_space(t.space_3);
                                }
                                ui.label(
                                    egui::RichText::new(h)
                                        .font(crate::theme::font(&t.type_body_strong))
                                        .color(t.text_primary.color32()),
                                );
                            }
                            NoticeBlock::Text(text) => {
                                ui.add(
                                    egui::Label::new(egui::RichText::new(reflow(text)).monospace()).wrap(),
                                );
                            }
                        }
                    }
                });
        }
        // WP-S2.08 part 2: the keymap editor.
        Dialog::Shortcuts => crate::keymap_editor::show(ui, shell, just_opened, events),
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
                let ago = strings::elapsed_text(*last_change_ms, dialog_now_ms(ui.ctx()));
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
        Dialog::PartChoose(c) => crate::part_dialog::chooser(ui, shell, c, just_opened, events),
        Dialog::PartEdit(e) => crate::part_dialog::editor(ui, e, just_opened, events),
        Dialog::ConfirmDiscard { .. } | Dialog::ConfirmCheckScheme { .. } | Dialog::ConfirmJournalDiscard => {
        }
    });
    let clicked = clicked.or(link_clicked);
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

/// The fields of a creation or renumber form, one row each (FT-558, FT-566): the label in the dialog size's fixed label
/// column (`size.dialog.label`, then `space.3`), so the field edges line up from one dialog to the next, and the field filling the
/// rest of the body, `size.control` high. A text field has no placeholder (its label names it); a choice shows doc
/// 16a §8's words ([`strings::choice_text`]) behind a `chevron-down`; a yes-or-no choice is a check box. `true` when
/// Enter in a text field submits the form.
fn form_fields(ui: &mut Ui, form: &FormDialog, just_opened: bool, events: &mut Vec<Event>) -> bool {
    let t = crate::theme::current(ui.ctx());
    let mut submit = false;
    let mut first = just_opened;
    // Cycle 39 (FIX39b-1): the label column is at least `size.dialog.label` and grows to the widest label, up to half
    // the body; a label wider still wraps, so a label is never cut ("Library (cannot be undone)").
    let font = egui::TextStyle::Body.resolve(ui.style());
    let widest = form
        .fields
        .iter()
        .map(|f| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(
                        strings::text(f.label_key).to_owned(),
                        font.clone(),
                        crate::theme::NONE,
                    )
                    .size()
                    .x
            })
        })
        .fold(0.0_f32, f32::max);
    let label_width = form_label_width(t.size_dialog_label, widest.ceil(), ui.available_width());
    for f in &form.fields {
        let label_text = strings::text(f.label_key);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let label = ui
                .allocate_ui_with_layout(
                    egui::vec2(label_width, t.size_control),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_size(egui::vec2(label_width, t.size_control));
                        ui.add(egui::Label::new(label_text).wrap())
                    },
                )
                .inner;
            // `space.3` between the label column and the field (doc 16a §6.8's label-to-value gap).
            ui.add_space(t.space_3);
            if f.is_toggle() {
                let mut on = f.value == "true";
                // At the field column's start, as the fields' text.
                let check = ui
                    .add(egui::Checkbox::without_text(&mut on))
                    .labelled_by(label.id);
                if check.changed() {
                    events.push(Event::FormField(f.name, on.to_string()));
                }
                if first {
                    check.request_focus();
                    first = false;
                }
            } else if f.choices.is_empty() {
                let mut text = f.value.clone();
                let edit = ui
                    .add(
                        TextEdit::singleline(&mut text)
                            .desired_width(ui.available_width())
                            .min_size(egui::vec2(0.0, t.size_control)),
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
                    .selected_text(strings::choice_text(&f.value))
                    .width(ui.available_width())
                    .icon(crate::components::combo_chevron)
                    .show_ui(ui, |ui| {
                        for choice in f.choices {
                            let word = strings::choice_text(choice);
                            if ui.selectable_label(f.value == *choice, word).clicked() {
                                events.push(Event::FormField(f.name, (*choice).to_owned()));
                            }
                        }
                    });
                combo.response.labelled_by(label.id);
            }
        });
    }
    // FT-786: Save as macro's value-set table under the fields, the refusal under it.
    if let Some(table) = &form.table {
        crate::value_table::show(ui, table, events);
    }
    if let Some(error) = &form.error {
        ui.colored_label(t.error.color32(), error);
    }
    submit
}

/// The width of a form's label column: `floor` (`size.dialog.label`), grown to the `widest` label, at most half of
/// the `body` (a label wider still wraps).
fn form_label_width(floor: f32, widest: f32, body: f32) -> f32 {
    widest.min(body / 2.0).max(floor)
}

/// One block of the notices text: an underlined heading, or the text up to the next heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NoticeBlock {
    Heading(String),
    Text(String),
}

/// The notices text as headings and text (FT-559): the SPDX header lines of each notices file are dropped (they are
/// the files' licensing, not notices), a line underlined by a line of "=" or "-" (three or more) becomes a heading
/// without its rule, and the blank lines at a block's ends are trimmed.
pub(crate) fn notice_blocks(text: &str) -> Vec<NoticeBlock> {
    fn rule(line: &str) -> bool {
        let t = line.trim();
        t.len() >= 3 && (t.chars().all(|c| c == '=') || t.chars().all(|c| c == '-'))
    }
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("SPDX-FileCopyrightText:") || t.starts_with("SPDX-License-Identifier:"))
        })
        .collect();
    let mut out = Vec::new();
    let mut body: Vec<&str> = Vec::new();
    let flush = |body: &mut Vec<&str>, out: &mut Vec<NoticeBlock>| {
        let text = body.join("\n");
        let text = text.trim_matches('\n');
        if !text.trim().is_empty() {
            out.push(NoticeBlock::Text(text.to_owned()));
        }
        body.clear();
    };
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if !line.trim().is_empty() && !rule(line) && lines.get(i + 1).is_some_and(|n| rule(n)) {
            flush(&mut body, &mut out);
            out.push(NoticeBlock::Heading(line.trim().to_owned()));
            i += 2;
            continue;
        }
        body.push(line);
        i += 1;
    }
    flush(&mut body, &mut out);
    out
}

/// The notices text with its hard line breaks inside a paragraph joined, so the label's own wrap is the only one (the
/// file's hard wrap wrapped again left orphan words such as "user"). A line keeps its break when it or the
/// previous line is empty, or when it starts a list item, an underline or an SPDX header line, or is indented (licence
/// texts keep their layout).
pub(crate) fn reflow(text: &str) -> String {
    fn keeps_break(line: &str) -> bool {
        let t = line.trim_start();
        line.len() != t.len()
            || t.starts_with(['-', '*', '•', '#', '='])
            || t.starts_with("SPDX-")
            || t.split_once(['.', ')']).is_some_and(|(n, _)| {
                !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_alphanumeric())
            })
    }
    let mut out = String::with_capacity(text.len());
    let mut prev_blank = true;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if !out.is_empty() {
            if blank || prev_blank || keeps_break(line) {
                out.push('\n');
            } else {
                out.push(' ');
            }
        }
        out.push_str(if blank { "" } else { line.trim_end() });
        prev_blank = blank;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{NoticeBlock, form_label_width, notice_blocks, reflow};

    /// Cycle 39 (FIX39b-1): the label column grows to its widest label, keeps its floor, stops at half the body.
    #[test]
    fn form_label_column_fits_its_widest_label() {
        assert_eq!(form_label_width(120.0, 80.0, 400.0), 120.0, "the floor");
        assert_eq!(form_label_width(120.0, 170.0, 400.0), 170.0, "the widest label");
        assert_eq!(
            form_label_width(120.0, 260.0, 400.0),
            200.0,
            "half the body; the label wraps"
        );
    }

    /// FT-559: no SPDX header line and no underline rule reaches the dialog; the underlined lines are headings.
    #[test]
    fn notice_blocks_drop_the_spdx_header_and_turn_rules_into_headings() {
        let text = "SPDX-FileCopyrightText: 2026 X\nSPDX-License-Identifier: CC0-1.0\n\nTitle\n=====\n\nIntro line.\n\n\
                    Fonts\n-----\n\nA font.\n  Indented: 1\n";
        assert_eq!(
            notice_blocks(text),
            [
                NoticeBlock::Heading("Title".into()),
                NoticeBlock::Text("Intro line.".into()),
                NoticeBlock::Heading("Fonts".into()),
                NoticeBlock::Text("A font.\n  Indented: 1".into()),
            ]
        );
    }

    #[test]
    fn the_notices_show_no_spdx_line_and_no_rule() {
        let blocks = notice_blocks(ew_viewmodel::about::NOTICE);
        assert!(matches!(&blocks[0], NoticeBlock::Heading(h) if h == "ElektraWatt third-party notices"));
        for b in &blocks {
            let (NoticeBlock::Heading(t) | NoticeBlock::Text(t)) = b;
            for line in t.lines() {
                assert!(!line.trim_start().starts_with("SPDX-"), "{line}");
                let l = line.trim();
                assert!(
                    !(l.len() >= 3 && (l.chars().all(|c| c == '=') || l.chars().all(|c| c == '-'))),
                    "{line}"
                );
            }
        }
    }

    #[test]
    fn reflow_joins_the_lines_of_a_paragraph() {
        let text =
            "This font is free software; you can give it to any\nuser under the licence.\n\nSecond para.";
        assert_eq!(
            reflow(text),
            "This font is free software; you can give it to any user under the licence.\n\nSecond para."
        );
    }

    #[test]
    fn reflow_keeps_list_items_and_indented_lines() {
        let text = "Terms:\n- one\n- two\n  indented\n1. first\nnext";
        assert_eq!(reflow(text), "Terms:\n- one\n- two\n  indented\n1. first next");
    }

    #[test]
    fn reflow_leaves_no_orphan_line_in_the_notices() {
        let out = reflow(ew_viewmodel::about::NOTICE);
        assert!(out.contains("a face of the user "), "{out}");
        assert!(!out.lines().any(|l| l.trim() == "user"), "{out}");
    }
}
