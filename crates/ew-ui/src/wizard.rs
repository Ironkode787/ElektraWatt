// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company-standard wizard screen (WP-S2.01 part 2, FT-308; UX/UI doc 16 section 9.2, FR-PRJ-010): the steps on
//! the left, the fields of the current step in the middle, and on the right the live preview, a cover or folio 102
//! of the sample project rendered by the CPU renderer with the current choices.
//!
//! Every control dispatches a `wizard.*` action. A text field commits when it loses the keyboard focus (Enter or a
//! click elsewhere), so the preview, which rebuilds from the sample project, follows each committed value and not
//! each key.

use egui::{Panel, TextEdit, Ui};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::wizard::{FieldKind, FieldMeta, Step, Wizard, fields_of};
use serde_json::{Value, json};

use crate::app::Event;
use crate::page_view::PageView;

fn set(events: &mut Vec<Event>, field: &str, value: &str) {
    events.push(Event::Action(
        "wizard.set",
        json!({ "field": field, "value": value }),
    ));
}

/// A text field that commits on losing the focus; the text being typed is kept in egui's temporary data.
fn text_field(ui: &mut Ui, events: &mut Vec<Event>, meta: &FieldMeta, value: &str) {
    let label = strings::text(meta.label_key);
    let id = ui.make_persistent_id(("wizard-field", meta.key));
    let shown: String = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.to_owned());
    let mut buffer = shown;
    let caption = ui.label(label);
    let edit = if meta.kind == FieldKind::Multiline {
        TextEdit::multiline(&mut buffer)
            .desired_rows(4)
            .desired_width(f32::INFINITY)
    } else {
        TextEdit::singleline(&mut buffer).desired_width(f32::INFINITY)
    };
    let response = ui.add(edit.hint_text(label)).labelled_by(caption.id);
    if response.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if buffer != value {
            set(events, meta.key, &buffer);
        }
    } else if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buffer));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
}

fn choice_field(ui: &mut Ui, events: &mut Vec<Event>, meta: &FieldMeta, choices: &[&str], value: &str) {
    ui.label(strings::text(meta.label_key));
    ui.horizontal_wrapped(|ui| {
        for choice in choices {
            let text = strings::text(&format!("choice-{}-{choice}", meta.key));
            if ui.selectable_label(*choice == value, text).clicked() && *choice != value {
                set(events, meta.key, choice);
            }
        }
    });
}

fn step_form(ui: &mut Ui, wizard: &Wizard, events: &mut Vec<Event>) {
    let step = wizard.step();
    ui.heading(format!("{}. {}", step.number(), strings::text(step.title_key())));
    ui.label(strings::text(step.hint_key()));
    ui.add_space(8.0);
    if step == Step::Review {
        egui::Grid::new("wizard-review")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for line in wizard.draft().review() {
                    ui.label(line.label);
                    ui.label(line.value);
                    ui.weak(strings::text(if line.written {
                        "wizard-written"
                    } else {
                        "wizard-kept"
                    }));
                    ui.end_row();
                }
            });
        return;
    }
    for meta in fields_of(step) {
        let value = wizard.draft().get(meta.key).to_owned();
        match meta.kind {
            FieldKind::Choice(choices) => choice_field(ui, events, meta, choices, &value),
            FieldKind::Text | FieldKind::Multiline => text_field(ui, events, meta, &value),
        }
        ui.add_space(6.0);
    }
}

/// Paints the wizard into `ui`; `preview` is the page view that draws the preview page.
pub fn show(ui: &mut Ui, shell: &ShellModel, preview: &mut PageView, events: &mut Vec<Event>) {
    let Some(wizard) = shell.wizard() else { return };
    Panel::left("wizard-steps")
        .resizable(false)
        .default_size(220.0)
        .show(ui, |ui| {
            ui.heading(strings::text("wizard-title"));
            for step in Step::ALL {
                let text = format!("{}. {}", step.number(), strings::text(step.title_key()));
                if ui.selectable_label(step == wizard.step(), text).clicked() && step != wizard.step() {
                    events.push(Event::Action("wizard.goto", json!({ "step": step.index() })));
                }
            }
        });
    Panel::right("wizard-preview")
        .resizable(true)
        .default_size(440.0)
        .show(ui, |ui| {
            ui.heading(strings::text("wizard-preview-title"));
            ui.horizontal(|ui| {
                for (cover, key) in [(true, "wizard-preview-cover"), (false, "wizard-preview-page")] {
                    let selected = wizard.preview_is_cover() == cover;
                    if ui.selectable_label(selected, strings::text(key)).clicked() && !selected {
                        set(events, "preview", if cover { "cover" } else { "page" });
                    }
                }
            });
            ui.separator();
            let folio = wizard.preview_folio();
            preview.show_source(ui, wizard.preview_rev(), folio, &|_| {
                shell.wizard_preview_content()
            });
        });
    egui::CentralPanel::default().show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            step_form(ui, wizard, events);
            if let Some(error) = wizard.error() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if wizard.step() != Step::Company && ui.button(strings::text("wizard-back")).clicked() {
                    events.push(Event::Action("wizard.back", Value::Null));
                }
                if wizard.step() == Step::Review {
                    if ui.button(strings::text("wizard-create")).clicked() {
                        events.push(Event::Action("wizard.finish", Value::Null));
                    }
                } else if ui.button(strings::text("wizard-next")).clicked() {
                    events.push(Event::Action("wizard.next", Value::Null));
                }
                if ui.button(strings::text("wizard-reset")).clicked() {
                    events.push(Event::Action("wizard.reset", Value::Null));
                }
                if ui.button(strings::text("wizard-cancel")).clicked() {
                    events.push(Event::Action("wizard.cancel", Value::Null));
                }
            });
        });
    });
}
