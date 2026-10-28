// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Jobs pane (WP-S2.09 part 1, §6.4): one entry per PDF export, oldest first, with the kind, the file's base name,
//! the revision it runs on, the progress (`page 12 of 36`, or the phase while it is indeterminate), the elapsed time
//! (the window's clock only) and Cancel while it runs; a finished entry keeps its result (the pages and size with Open,
//! "cancelled; no file written", or the failure's cause).
//!
//! **Accessibility.** The pane is a `Group` named "Jobs"; a running entry is a `ProgressIndicator` named `<kind>
//! <name>` with its value in per cent; the buttons are named by their text.

use std::collections::BTreeMap;
use std::time::Instant;

use egui::Ui;
use ew_viewmodel::jobs::{JobEntry, JobStatus};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::{status, strings};
use serde_json::json;

use crate::app::Event;

/// Paints the Jobs pane of `shell`: `started` holds the window's start time of each running job, `now` the frame's.
pub fn show(
    ui: &mut Ui,
    shell: &ShellModel,
    started: &BTreeMap<u64, Instant>,
    now: Instant,
    events: &mut Vec<Event>,
) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("jobs-pane");
    let pane = ui.interact(rect, pane_id, egui::Sense::hover());
    pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("pane-jobs")));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        let jobs = shell.jobs();
        if jobs.entries().next().is_none() {
            crate::components::empty_state(
                ui,
                crate::icons::IconId::file_type_pdf,
                strings::text("empty-jobs"),
                strings::text("empty-jobs-text"),
                None,
            );
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for e in jobs.entries() {
                    entry(
                        ui,
                        e,
                        started.get(&e.number).map(|t| now.saturating_duration_since(*t)),
                        events,
                    );
                    ui.separator();
                }
            });
    });
}

/// One entry: its heading line, then the progress and Cancel, or the result.
fn entry(ui: &mut Ui, e: &JobEntry, elapsed: Option<std::time::Duration>, events: &mut Vec<Event>) {
    let kind = strings::text(e.kind.label_key());
    let name = format!("{kind} {}", e.name);
    ui.horizontal_wrapped(|ui| {
        ui.strong(&name);
        ui.label(strings::format("job-as-of", &[&e.rev.to_string()]));
    });
    match &e.status {
        JobStatus::Running => {
            let progress = e.progress().unwrap_or_default();
            let (fraction, text) = if progress.total > 0 {
                (
                    progress.done as f32 / progress.total as f32,
                    strings::format(
                        "job-progress",
                        &[&progress.done.to_string(), &progress.total.to_string()],
                    ),
                )
            } else {
                (0.0, status::phase_text(progress.phase))
            };
            ui.horizontal(|ui| {
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(220.0)
                        .animate(progress.total == 0)
                        .text(name.clone()),
                );
                ui.label(text);
                if let Some(d) = elapsed {
                    ui.label(strings::format("job-elapsed", &[&d.as_secs().to_string()]));
                }
                if ui.button(strings::text("job-cancel")).clicked() {
                    events.push(Event::Action("jobs.cancel", json!({ "job": e.number })));
                }
            });
        }
        // WP-S2.17 part 4 (FT-510, doc 16a §6.19): a finished entry names its outcome as a status badge next to its
        // result.
        JobStatus::Done { pages, kib, now } => {
            ui.horizontal_wrapped(|ui| {
                crate::components::status_badge(ui, crate::components::Badge::Done);
                ui.label(strings::format(
                    "job-done",
                    &[&pages.to_string(), &kib.to_string()],
                ));
                if let Some(m) = now {
                    ui.label(strings::format("job-now", &[&e.rev.to_string(), &m.to_string()]));
                }
                if ui.button(strings::text("toast-open")).clicked() {
                    events.push(Event::Action("jobs.open_result", json!({ "job": e.number })));
                }
            });
        }
        JobStatus::Cancelled => {
            ui.horizontal_wrapped(|ui| {
                crate::components::status_badge(ui, crate::components::Badge::Cancelled);
                ui.label(strings::text("job-cancelled"));
            });
        }
        JobStatus::Failed(cause) => {
            ui.horizontal_wrapped(|ui| {
                crate::components::status_badge(ui, crate::components::Badge::Failed);
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    strings::format("job-failed", &[cause]),
                );
            });
        }
    }
}
