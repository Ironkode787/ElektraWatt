// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The first-run screen (WP-S2.01 part 2, FT-308; UX/UI doc 16 section 9.1, FR-UI-030): language, what the user
//! comes from, the privacy choice, the appearance, and where to start. Every step can be skipped. It makes no
//! network connection. The answers are dispatched as `first_run.set`; the flow ends with `first_run.finish`.

use egui::Ui;
use ew_viewmodel::first_run::FirstRunStep;
use ew_viewmodel::settings::{LANGUAGES, ORIGINS, THEMES, UI_SCALES};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

fn set(events: &mut Vec<Event>, field: &str, value: Value) {
    events.push(Event::Action(
        "first_run.set",
        json!({ "field": field, "value": value }),
    ));
}

fn choices(ui: &mut Ui, events: &mut Vec<Event>, field: &str, prefix: &str, options: &[&str], current: &str) {
    for option in options {
        let text = strings::text(&format!("{prefix}-{option}"));
        if ui.selectable_label(*option == current, text).clicked() && *option != current {
            set(events, field, json!(option));
        }
    }
}

/// Paints the first-run screen into `ui`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(run) = shell.first_run() else { return };
    let settings = shell.settings();
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.1);
        ui.heading(strings::text("firstrun-title"));
        ui.label(format!(
            "{} / {}: {}",
            run.step.index() + 1,
            FirstRunStep::ALL.len(),
            strings::text(run.step.title_key())
        ));
        ui.add_space(12.0);
        ui.scope(|ui| {
            ui.set_max_width(520.0);
            match run.step {
                FirstRunStep::Language => {
                    ui.label(strings::text("firstrun-text-language"));
                    ui.horizontal(|ui| {
                        choices(ui, events, "language", "language", &LANGUAGES, &settings.language);
                    });
                }
                FirstRunStep::Origin => {
                    ui.label(strings::text("firstrun-text-origin"));
                    ui.horizontal(|ui| {
                        choices(ui, events, "comes_from", "origin", &ORIGINS, &settings.comes_from);
                    });
                }
                FirstRunStep::Privacy => {
                    ui.label(strings::text("firstrun-text-privacy"));
                    let mut on = settings.update_check;
                    if ui
                        .checkbox(&mut on, strings::text("firstrun-update-check"))
                        .changed()
                    {
                        set(events, "update_check", json!(on));
                    }
                }
                FirstRunStep::Appearance => {
                    ui.label(strings::text("firstrun-text-appearance"));
                    ui.horizontal(|ui| {
                        ui.label(strings::text("firstrun-theme"));
                        choices(ui, events, "theme", "theme", &THEMES, &settings.theme);
                    });
                    ui.horizontal(|ui| {
                        ui.label(strings::text("firstrun-scale"));
                        for (key, scale) in ["scale-100", "scale-125", "scale-150", "scale-200"]
                            .iter()
                            .zip(UI_SCALES)
                        {
                            let current = (settings.ui_scale - scale).abs() < 1e-3;
                            if ui.selectable_label(current, strings::text(key)).clicked() && !current {
                                set(events, "ui_scale", json!(scale));
                            }
                        }
                    });
                }
                FirstRunStep::Start => {
                    ui.label(strings::text("firstrun-text-start"));
                    for (key, start) in [
                        ("firstrun-start-wizard", "wizard"),
                        ("firstrun-start-sample", "sample"),
                        ("firstrun-start-qet", "qet"),
                        ("firstrun-start-new", "new"),
                        ("firstrun-start-none", "none"),
                    ] {
                        if ui.button(strings::text(key)).clicked() {
                            events.push(Event::Action("first_run.finish", json!({ "start": start })));
                        }
                    }
                }
            }
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if run.step != FirstRunStep::Language && ui.button(strings::text("firstrun-back")).clicked() {
                events.push(Event::Action("first_run.back", Value::Null));
            }
            if run.step != FirstRunStep::Start {
                if ui.button(strings::text("firstrun-skip")).clicked() {
                    events.push(Event::Action("first_run.next", Value::Null));
                }
                if ui.button(strings::text("firstrun-next")).clicked() {
                    events.push(Event::Action("first_run.next", Value::Null));
                }
            }
        });
    });
}
