// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The first-run screen (UX/UI doc 16 section 9.1, FR-UI-030): language, what the user
//! comes from, the privacy choice, the appearance, and where to start. Every step can be skipped. It makes no
//! network connection. The answers are dispatched as `first_run.set`; the flow ends with `first_run.finish`.

use egui::Ui;
use ew_viewmodel::first_run::FirstRunStep;
use ew_viewmodel::key;
use ew_viewmodel::settings::{LANGUAGES, ORIGINS, THEMES, UI_SCALES, scale_label};
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
        let text = strings::text(strings::Key::run_time(&format!("{prefix}-{option}")));
        if ui.selectable_label(*option == current, text).clicked() && *option != current {
            set(events, field, json!(option));
        }
    }
}

/// The Start step's buttons and the start each one takes (the screen and the fit test read this one list).
pub const STARTS: [(strings::Key<'static>, &str); 5] = [
    (key!("firstrun-start-wizard"), "wizard"),
    (key!("firstrun-start-sample"), "sample"),
    (key!("firstrun-start-qet"), "qet"),
    (key!("firstrun-start-new"), "new"),
    (key!("firstrun-start-none"), "none"),
];

/// The footer's buttons on `step`, in their order, with the action each runs: Back after the first step, Skip and
/// Next before the last (the screen and the fit test read this one list).
pub fn footer(step: FirstRunStep) -> Vec<(strings::Key<'static>, &'static str)> {
    let mut out = Vec::new();
    if step != FirstRunStep::Language {
        out.push((key!("firstrun-back"), "first_run.back"));
    }
    if step != FirstRunStep::Start {
        out.push((key!("firstrun-skip"), "first_run.next"));
        out.push((key!("firstrun-next"), "first_run.next"));
    }
    out
}

/// Paints the first-run screen into `ui`.
pub(crate) fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(run) = shell.first_run() else { return };
    let settings = shell.settings();
    // The reading measure, kept fixed on purpose (running text gets no easier to read wider). Doc 16a §2:
    // the heading, the texts, the choices and the buttons share that one column, centred in the window.
    let width = crate::theme::current(ui.ctx())
        .size_text_measure
        .min(ui.available_width());
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.1);
        let column = egui::vec2(width, ui.available_height());
        ui.allocate_ui_with_layout(column, egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(width);
            ui.heading(strings::text(key!("firstrun-title")));
            ui.label(format!(
                "{} / {}: {}",
                run.step.index() + 1,
                FirstRunStep::ALL.len(),
                strings::text(run.step.title_key())
            ));
            ui.add_space(12.0);
            match run.step {
                FirstRunStep::Language => {
                    ui.label(strings::text(key!("firstrun-text-language")));
                    ui.horizontal(|ui| {
                        choices(ui, events, "language", "language", &LANGUAGES, &settings.language);
                    });
                }
                FirstRunStep::Origin => {
                    ui.label(strings::text(key!("firstrun-text-origin")));
                    ui.horizontal(|ui| {
                        choices(ui, events, "comes_from", "origin", &ORIGINS, &settings.comes_from);
                    });
                }
                FirstRunStep::Privacy => {
                    // This version has no update check, so no box for one.
                    ui.label(strings::text(key!("firstrun-text-privacy")));
                    if ui.button(strings::text(key!("about-privacy-button"))).clicked() {
                        events.push(Event::OpenPrivacy);
                    }
                }
                FirstRunStep::Appearance => {
                    ui.label(strings::text(key!("firstrun-text-appearance")));
                    ui.horizontal(|ui| {
                        ui.label(strings::text(key!("firstrun-theme")));
                        choices(ui, events, "theme", "theme", &THEMES, &settings.theme);
                    });
                    ui.horizontal(|ui| {
                        ui.label(strings::text(key!("firstrun-scale")));
                        for scale in UI_SCALES {
                            let current = (settings.ui_scale - scale).abs() < 1e-3;
                            if ui.selectable_label(current, scale_label(scale)).clicked() && !current {
                                set(events, "ui_scale", json!(scale));
                            }
                        }
                    });
                }
                FirstRunStep::Start => {
                    ui.label(strings::text(key!("firstrun-text-start")));
                    for (key, start) in STARTS {
                        if ui.button(strings::text(key)).clicked() {
                            events.push(Event::Action("first_run.finish", json!({ "start": start })));
                        }
                    }
                }
            }
            ui.add_space(16.0);
            // Back and Skip at the column's left edge, Next at its right.
            let buttons = footer(run.step);
            let (left, next) = match buttons.split_last() {
                Some((next, left)) if run.step != FirstRunStep::Start => (left, Some(next)),
                _ => (buttons.as_slice(), None),
            };
            ui.horizontal(|ui| {
                for (key, action) in left {
                    if ui.button(strings::text(*key)).clicked() {
                        events.push(Event::Action(action, Value::Null));
                    }
                }
                if let Some((key, action)) = next {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(strings::text(*key)).clicked() {
                            events.push(Event::Action(action, Value::Null));
                        }
                    });
                }
            });
        });
    });
}
