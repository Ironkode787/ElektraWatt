// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The find pane (WP-S2.05 part 2, spec §7.3 and §7.4): the search text, the replacement (find and replace), the
//! toggles, the field checkboxes, the folio filter, the results with their matches marked, and the replace card.
//!
//! **Input.** Every gesture is a registry action (`find.*`, `preview.*`); the pane builds no command. In the text,
//! the replacement and the folio fields Enter runs the search (`find.run`; typing alone does not search) and
//! Ctrl+Enter replaces (`find.replace_all` builds the card, the next one applies it as `preview.apply`).
//! While the pane has the shell's focus and no field is edited: Up, Down, Home and End move the result cursor, Enter shows the result (`find.jump`),
//! Ctrl+Enter replaces or applies, Escape goes back to the canvas (closing the card first).
//!
//! **Accessibility.** The pane is a `Group` named "Find and replace"; the fields are text inputs and the toggles check
//! boxes labelled by their names; the results are a list box whose options are named by the result's line, the
//! matches in `[[…]]` (never colour alone: the window also draws them bold and underlined); the card and the pane's
//! note are polite live regions.

use egui::text::{LayoutJob, TextFormat};
use egui::{Key, Modifiers, Sense, TextEdit, Ui};
use ew_viewmodel::find::{FIELDS, FindHit, field_label_key, field_word, hit_label, marked};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// The find pane's state between frames.
#[derive(Default)]
pub struct FindPane {
    text: String,
    replacement: String,
    folios: String,
    /// Whether the pane had the shell's focus in the last frame (the text field takes the focus when it gets it).
    was_ours: bool,
}

/// The text of a result line: folio and cell, subject, field, the string with its matches marked, the replacement or
/// why it is not replaced.
pub fn result_line(hit: &FindHit) -> String {
    let mut line = String::new();
    if let Some(cell) = hit.cell.as_deref().or(hit.folio.as_deref()) {
        line.push_str(cell);
        line.push_str(" · ");
    }
    line.push_str(&format!("{} · {} · {}", hit.subject, hit_label(hit), marked(hit)));
    match (&hit.after, &hit.replaceable) {
        (Some(after), _) => line.push_str(&format!(" -> {after}")),
        (None, Err(why)) => line.push_str(&format!(" ({})", strings::format("find-not-replaced", &[why]))),
        (None, Ok(())) => {}
    }
    line
}

/// Whether the replace card of the find pane is open.
fn has_find_card(shell: &ShellModel) -> bool {
    shell
        .properties()
        .card()
        .is_some_and(|c| matches!(c.origin, CardOrigin::Find { .. }))
}

impl FindPane {
    /// A pane with empty fields.
    pub fn new() -> FindPane {
        FindPane::default()
    }

    /// Paints the find pane of `shell` into `ui`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let f = shell.find_state();
        let rect = ui.available_rect_before_wrap();
        let pane_id = ui.scope_id().with("find-pane");
        let pane = ui.interact(rect, pane_id, Sense::focusable_noninteractive());
        let pane_name = strings::text("pane-find").to_owned();
        pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &pane_name));
        let ours = shell.focus() == Some(Focus::Find);
        let arrived = ours && !self.was_ours;
        self.was_ours = ours;
        if ours && !arrived && !pane.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none()
        {
            pane.request_focus();
        }
        if pane.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    pane_id,
                    egui::EventFilter {
                        horizontal_arrows: false,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                );
            });
            Self::keys(ui, shell, events);
        }
        if pane.gained_focus() && !ours {
            events.push(Event::Action("ui.focus", json!({"area": "find"})));
        }
        // The fields follow the shell when they are not being typed into (a script or another window changed them).
        if !ui.ctx().text_edit_focused() {
            self.text = f.text().to_owned();
            self.replacement = f.replacement().to_owned();
            self.folios = f.folios().join(", ");
        }
        let card = has_find_card(shell);
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
            Self::card(ui, shell, events);
            ui.horizontal_wrapped(|ui| {
                let name = strings::text("find-text").to_owned();
                let response = Self::field(ui, &mut self.text, &name, events, "find.text", (card, ours));
                if arrived {
                    response.request_focus();
                }
                if f.replace_mode() {
                    let name = strings::text("find-replacement").to_owned();
                    Self::field(
                        ui,
                        &mut self.replacement,
                        &name,
                        events,
                        "find.replacement",
                        (card, ours),
                    );
                }
                if ui.button(strings::text("find-run")).clicked() {
                    events.push(Event::Action("find.run", json!({})));
                }
                if f.replace_mode() && ui.button(strings::text("find-replace-all")).clicked() {
                    events.push(Event::Action("find.replace_all", json!({})));
                }
                if ui.button(strings::text("find-close")).clicked() {
                    events.push(Event::Action("find.close", json!({})));
                }
            });
            ui.horizontal_wrapped(|ui| {
                let (regex, case, word, selection) = f.options();
                for (option, on, key) in [
                    ("regex", regex, "find-regex"),
                    ("match_case", case, "find-match-case"),
                    ("whole_word", word, "find-whole-word"),
                    ("in_selection", selection, "find-in-selection"),
                ] {
                    let mut now = on;
                    let name = strings::text(key).to_owned();
                    let response = ui.checkbox(&mut now, &name);
                    response
                        .widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &name));
                    if now != on {
                        events.push(Event::Action("find.option", json!({"option": option, "on": now})));
                    }
                }
                ui.separator();
                for field in FIELDS {
                    let on = f.fields().contains(&field);
                    let mut now = on;
                    let name = strings::text(field_label_key(field)).to_owned();
                    let response = ui.checkbox(&mut now, &name);
                    response
                        .widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, &name));
                    if now != on {
                        let fields: Vec<&str> = FIELDS
                            .into_iter()
                            .filter(|x| if *x == field { now } else { f.fields().contains(x) })
                            .map(field_word)
                            .collect();
                        events.push(Event::Action("find.fields", json!({ "fields": fields })));
                    }
                }
                ui.separator();
                let name = strings::text("find-folios").to_owned();
                let response = ui.add(
                    TextEdit::singleline(&mut self.folios)
                        .desired_width(140.0)
                        .hint_text(&name),
                );
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
                Self::report_focus(&response, ours, events);
                if response.changed() {
                    let folios: Vec<&str> = self
                        .folios
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect();
                    events.push(Event::Action("find.folios", json!({ "folios": folios })));
                }
                Self::enter_keys(ui, &response, events, card);
            });
            Self::status(ui, shell);
            Self::results(ui, shell, events);
        });
    }

    /// A field that gains the focus (a click, Tab) makes the pane the shell's focus (see `focus.rs`).
    fn report_focus(response: &egui::Response, ours: bool, events: &mut Vec<Event>) {
        crate::focus::report_focus(response, ours, "find", events);
    }

    /// A text field labelled `name` whose changes go to `action`; Enter runs the search, Ctrl+Enter replaces.
    fn field(
        ui: &mut Ui,
        text: &mut String,
        name: &str,
        events: &mut Vec<Event>,
        action: &'static str,
        (card, ours): (bool, bool),
    ) -> egui::Response {
        let response = ui.add(TextEdit::singleline(text).desired_width(220.0).hint_text(name));
        Self::report_focus(&response, ours, events);
        let label = name.to_owned();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &label));
        if response.changed() {
            events.push(Event::Action(action, json!({ "text": text.as_str() })));
        }
        Self::enter_keys(ui, &response, events, card);
        response
    }

    /// Enter in a field runs the search; Ctrl+Enter replaces: the first builds the card (`find.replace_all`), the
    /// next applies it (`preview.apply`), as with no field focused and in `app.rs`.
    fn enter_keys(ui: &Ui, response: &egui::Response, events: &mut Vec<Event>, card: bool) {
        if response.has_focus()
            && ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter))
        {
            let action = if card { "preview.apply" } else { "find.replace_all" };
            events.push(Event::Action(action, json!({})));
        } else if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            events.push(Event::Action("find.run", json!({})));
        }
    }

    /// The pane's keys while it has the focus and no field is edited.
    fn keys(ui: &Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        let card = has_find_card(shell);
        if pressed(Modifiers::COMMAND, Key::Enter) {
            let action = if card { "preview.apply" } else { "find.replace_all" };
            events.push(Event::Action(action, json!({})));
        } else if pressed(Modifiers::NONE, Key::ArrowDown) {
            events.push(Event::Action("find.row", json!({"step": 1})));
        } else if pressed(Modifiers::NONE, Key::ArrowUp) {
            events.push(Event::Action("find.row", json!({"step": -1})));
        } else if pressed(Modifiers::NONE, Key::Home) {
            events.push(Event::Action("find.row", json!({"to": 1})));
        } else if pressed(Modifiers::NONE, Key::End) {
            let n = shell.find_state().results().map_or(0, |r| r.hits.len());
            if n > 0 {
                events.push(Event::Action("find.row", json!({ "to": n })));
            }
        } else if pressed(Modifiers::NONE, Key::Enter) {
            events.push(Event::Action("find.jump", json!({})));
        }
    }

    /// The replace card (§7.4): a polite live region with Apply and Cancel.
    fn card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
            return;
        };
        if !matches!(card.origin, CardOrigin::Find { .. }) {
            return;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            let mut line = strings::format(
                "find-card-line",
                &[
                    &card.changes.to_string(),
                    &card.targets.to_string(),
                    &card.preview.derived.tags.len().to_string(),
                    &card.preview.derived.xrefs.len().to_string(),
                ],
            );
            match (&card.preview.result, card.preview.status(rev)) {
                (Err(m), _) => line = strings::format("card-failed", &[m]),
                (_, ew_viewmodel::tools::PreviewStatus::Stale) => {
                    line.push_str(&format!(" [{}]", strings::text("card-stale")));
                }
                _ => {}
            }
            // WP-S2.09 part 3 (§8.1): while the Preview tab shows the card, a one-line chip stands in for it.
            if crate::preview::chip(ui, shell, &line, events) {
                return;
            }
            let response = ui.label(&line);
            let live = line.clone();
            ui.ctx().accesskit_node_builder(response.id, |n| {
                n.set_live(egui::accesskit::Live::Polite);
                n.set_value(live);
            });
            for tag in card.preview.derived.tags.iter().take(20) {
                ui.label(format!("{} -> {}", tag.before, tag.after));
            }
            ui.horizontal(|ui| {
                if ui.button(strings::text("card-apply")).clicked() {
                    events.push(Event::Action("preview.apply", json!({})));
                }
                if ui.button(strings::text("card-cancel")).clicked() {
                    events.push(Event::Action("preview.cancel", json!({})));
                }
            });
        });
    }

    /// The results' count, the stale badge, the error, the truncation and the note.
    fn status(ui: &mut Ui, shell: &ShellModel) {
        let f = shell.find_state();
        let mut line = match f.results() {
            None => String::new(),
            Some(r) if r.hits.is_empty() && r.error.is_none() => strings::text("find-no-results").to_owned(),
            Some(r) => strings::format("find-results", &[&r.hits.len().to_string()]),
        };
        if f.stale(shell.revision()) {
            line.push_str(&format!(" [{}]", strings::text("find-stale")));
        }
        if let Some(r) = f.results() {
            if let Some(e) = &r.error {
                line = e.clone();
            }
            if r.truncated {
                line.push_str(&format!(" · {}", strings::text("find-truncated")));
            }
        }
        if let Some(note) = f.note() {
            line.push_str(&format!(" · {note}"));
        }
        if line.is_empty() {
            return;
        }
        let response = ui.label(&line);
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(line);
        });
    }

    /// The results: a list box of one line per hit, the matches bold and underlined, the cursor's line selected.
    fn results(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let f = shell.find_state();
        let Some(r) = f.results() else {
            return;
        };
        let list_id = ui.scope_id().with("find-results");
        let list_rect = ui.available_rect_before_wrap();
        let list = ui.interact(
            egui::Rect::from_min_size(list_rect.min, egui::Vec2::new(list_rect.width(), 1.0)),
            list_id,
            Sense::hover(),
        );
        let list_name = strings::format("find-results", &[&r.hits.len().to_string()]);
        list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, &list_name));
        let row = f.row();
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("find-results-scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (i, hit) in r.hits.iter().enumerate() {
                        let selected = row == Some(i);
                        let response = ui.add(egui::Button::selectable(selected, Self::job(ui, hit)));
                        let name = result_line(hit);
                        response.widget_info(|| {
                            egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &name)
                        });
                        if selected {
                            response.scroll_to_me(None);
                        }
                        if response.double_clicked() {
                            events.push(Event::Action("ui.focus", json!({"area": "find"})));
                            events.push(Event::Action("find.row", json!({ "to": i + 1 })));
                            events.push(Event::Action("find.jump", json!({})));
                        } else if response.clicked() {
                            events.push(Event::Action("ui.focus", json!({"area": "find"})));
                            events.push(Event::Action("find.row", json!({ "to": i + 1 })));
                        }
                    }
                });
        });
    }

    /// A result's line as a layout job: the matches bold (strong colour) and underlined, the rest plain.
    fn job(ui: &Ui, hit: &FindHit) -> LayoutJob {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let plain = TextFormat::simple(font.clone(), ui.visuals().text_color());
        let t = crate::theme::current(ui.ctx());
        let strong = TextFormat {
            underline: egui::Stroke::new(t.border_width, ui.visuals().strong_text_color()),
            color: ui.visuals().strong_text_color(),
            ..TextFormat::simple(font, ui.visuals().strong_text_color())
        };
        let mut job = LayoutJob::default();
        let mut head = String::new();
        if let Some(cell) = hit.cell.as_deref().or(hit.folio.as_deref()) {
            head.push_str(cell);
            head.push_str(" · ");
        }
        head.push_str(&format!("{} · {} · ", hit.subject, hit_label(hit)));
        job.append(&head, 0.0, plain.clone());
        let mut at = 0;
        for (s, e) in &hit.ranges {
            if *s < at || *e > hit.before.len() {
                continue;
            }
            job.append(&hit.before[at..*s], 0.0, plain.clone());
            job.append(&hit.before[*s..*e], 0.0, strong.clone());
            at = *e;
        }
        job.append(&hit.before[at..], 0.0, plain.clone());
        match (&hit.after, &hit.replaceable) {
            (Some(after), _) => job.append(&format!(" -> {after}"), 0.0, plain),
            (None, Err(why)) => job.append(
                &format!(" ({})", strings::format("find-not-replaced", &[why])),
                0.0,
                plain,
            ),
            (None, Ok(())) => {}
        }
        job
    }
}
