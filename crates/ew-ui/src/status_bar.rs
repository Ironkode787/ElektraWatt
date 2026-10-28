// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status bar (WP-S2.01 part 1, spec section 5.3; UX/UI doc 16 section 3.6): the fields of
//! `ShellModel::status_fields`, left to right. The notifications field opens the log and the job field the Jobs pane
//! (WP-S2.09 part 1, §6.4, §6.5): both are links.

use egui::Ui;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

use crate::app::Event;

/// Paints the status bar of `shell` into `ui`; `hide_job` leaves the job field out (a job younger than 200 ms).
pub fn show(ui: &mut Ui, shell: &ShellModel, hide_job: bool, events: &mut Vec<Event>) {
    let fields = shell.status_fields(hide_job);
    let job = !hide_job && shell.jobs().newest_running().is_some();
    let notifications = !shell.notifications().is_empty();
    // The two fields at the end: the notifications field, then the job field.
    let job_at = job.then(|| fields.len() - 1);
    let log_at = notifications.then(|| fields.len() - 1 - usize::from(job));
    ui.horizontal(|ui| {
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            if Some(i) == log_at {
                if ui.link(field.as_str()).clicked() {
                    events.push(Event::Action("notifications.show", json!({})));
                }
            } else if Some(i) == job_at {
                if ui.link(field.as_str()).clicked() {
                    events.push(Event::Action("jobs.show", json!({ "on": true })));
                }
            } else {
                ui.label(field.as_str());
            }
        }
    });
}
