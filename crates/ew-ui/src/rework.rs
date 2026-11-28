// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Rework list… dialog's own content (doc 13 §3.5, §12; doc 16a §6.12, §6.21): Compare with (the releases whose
//! archive is found, newest first, and Archive file…), one line that counts the work, the rows under the headings of a
//! table, and the line of what is not listed; or why there is no list. Export… and Close are the dialog's buttons.

use egui::Ui;
use ew_viewmodel::key;
use ew_viewmodel::shell::rework::{Against, ReworkDialog};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components;

/// The share of the window's height the rows take before they scroll.
const ROWS_HEIGHT_SHARE: f32 = 0.5;

/// The dialog's content.
pub(crate) fn body(ui: &mut Ui, d: &ReworkDialog, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    ui.horizontal(|ui| {
        ui.label(components::body_secondary(
            &t,
            strings::text(key!("rework-compare-with")),
        ));
        let chosen = match &d.against {
            Against::Release(i) => d.releases.get(*i).map(|(_, w)| w.clone()).unwrap_or_default(),
            Against::File(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            Against::Nothing => String::new(),
        };
        if !d.releases.is_empty() {
            egui::ComboBox::from_id_salt("rework-compare-with")
                .selected_text(&chosen)
                .show_ui(ui, |ui| {
                    for (i, (release, words)) in d.releases.iter().enumerate() {
                        let on = d.against == Against::Release(i);
                        if ui.selectable_label(on, words).clicked() && !on {
                            events.push(Event::Action("rework.show", json!({ "revision": release.label })));
                        }
                    }
                });
        }
        if ui.button(strings::text(key!("rework-archive-file"))).clicked() {
            events.push(Event::Action("rework.archive", json!({})));
        }
        // What it compares with by its name alone: the release's label, the archive file's name.
        let name = match &d.against {
            Against::Release(i) => d.releases.get(*i).map(|(r, _)| r.label.clone()),
            Against::File(_) => Some(chosen.clone()),
            Against::Nothing => None,
        };
        if let (Some(name), true) = (name, d.list.is_ok()) {
            ui.label(components::small_secondary(
                &t,
                strings::format(key!("rework-against"), &[&name]),
            ));
        }
    });
    ui.add_space(t.space_3);
    let list = match &d.list {
        Ok(list) => list,
        Err(why) => {
            ui.add(egui::Label::new(components::body(&t, why)).wrap());
            return;
        }
    };
    ui.add(egui::Label::new(components::body(&t, &list.work)).wrap());
    if list.rows.is_empty() {
        return;
    }
    ui.add_space(t.space_2);
    let room = ui.ctx().content_rect().height() * ROWS_HEIGHT_SHARE;
    egui::ScrollArea::both()
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("rework-rows")
                .num_columns(list.header.len())
                .spacing(egui::vec2(t.space_4, t.space_2))
                .show(ui, |ui| {
                    for heading in &list.header {
                        ui.label(components::small_secondary(&t, heading).strong());
                    }
                    ui.end_row();
                    for row in &list.rows {
                        for (i, cell) in row.cells.iter().enumerate() {
                            // The action leads the row; the change takes the rest of the width, and why the row is
                            // there stands under it.
                            if i == 0 {
                                ui.label(components::body_strong(&t, cell));
                            } else if i + 1 == row.cells.len() {
                                ui.vertical(|ui| {
                                    ui.add(egui::Label::new(components::body(&t, cell)).wrap());
                                    if !row.because.is_empty() {
                                        let line =
                                            strings::format(key!("rework-because-line"), &[&row.because]);
                                        ui.add(egui::Label::new(components::body_secondary(&t, line)).wrap());
                                    }
                                });
                            } else {
                                ui.label(components::body(&t, cell));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    ui.add_space(t.space_2);
    ui.add(
        egui::Label::new(components::small_secondary(
            &t,
            strings::text(key!("rework-not-listed")),
        ))
        .wrap(),
    );
    // The label files of these changes (doc 13 §5.4): Labels… on what the list compares with.
    let against = match &d.against {
        Against::Release(i) => d.releases.get(*i).map(|(r, _)| json!({ "since": r.label })),
        Against::File(path) => Some(json!({ "archive": path.to_string_lossy() })),
        Against::Nothing => None,
    };
    if let Some(params) = against
        && ui.button(strings::text(key!("rework-labels"))).clicked()
    {
        events.push(Event::Action("labels.show", params));
    }
}
