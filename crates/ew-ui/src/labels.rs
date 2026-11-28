// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Labels… dialog's own content (doc 13 §5.4, §12; doc 16a §6.12): which labels (every one, or those changed since
//! a release), the label file's export profile in a line with Change…, one row per label file with its tick, label set,
//! marker part and count, and the line that counts the labels that do not fit; or why there are no files. Export… and
//! Close are the dialog's buttons.

use egui::Ui;
use ew_viewmodel::key;
use ew_viewmodel::shell::labels::{LabelsDialog, Which};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::components::Tip;

/// The share of the window's height the files take before they scroll.
const ROWS_HEIGHT_SHARE: f32 = 0.4;

/// The dialog's content.
pub(crate) fn body(ui: &mut Ui, d: &LabelsDialog, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    egui::Grid::new("labels-choices")
        .num_columns(2)
        .spacing(egui::vec2(t.space_4, t.space_2))
        .show(ui, |ui| {
            let which_label = ui.label(components::body_secondary(
                &t,
                strings::text(key!("labels-which")),
            ));
            let chosen = match &d.which {
                Which::All => strings::text(key!("labels-all")).to_owned(),
                Which::Release(i) => d
                    .releases
                    .get(*i)
                    .map(|(r, _)| strings::format(key!("labels-since"), &[&r.label]))
                    .unwrap_or_default(),
                Which::File(path) => strings::format(
                    key!("labels-archive"),
                    &[&path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()],
                ),
            };
            let combo = components::choice(
                ui,
                egui::ComboBox::from_id_salt("labels-which").selected_text(&chosen),
                |ui| {
                    if ui
                        .selectable_label(d.which == Which::All, strings::text(key!("labels-all")))
                        .clicked()
                        && d.which != Which::All
                    {
                        events.push(Event::Action("labels.show", json!({ "since": "" })));
                    }
                    for (i, (release, _)) in d.releases.iter().enumerate() {
                        let on = d.which == Which::Release(i);
                        let words = strings::format(key!("labels-since"), &[&release.label]);
                        if ui.selectable_label(on, words).clicked() && !on {
                            events.push(Event::Action("labels.show", json!({ "since": release.label })));
                        }
                    }
                    // Without a release its place says why it cannot be chosen.
                    if let Some(note) = d.since_note() {
                        ui.add_enabled(false, egui::Button::selectable(false, note))
                            .disabled_tip(note);
                    }
                },
            );
            combo.response.labelled_by(which_label.id);
            ui.end_row();
            ui.label(components::body_secondary(
                &t,
                strings::text(key!("labels-profile")),
            ));
            ui.horizontal(|ui| {
                ui.label(components::body(&t, &d.profile));
                if ui.button(strings::text(key!("labels-change"))).clicked() {
                    events.push(Event::Action("labels.columns", json!({})));
                }
            });
            ui.end_row();
        });
    ui.add_space(t.space_3);
    let files = match &d.files {
        Ok(files) => files,
        Err(why) => {
            ui.add(egui::Label::new(components::body(&t, why)).wrap());
            return;
        }
    };
    let room = ui.ctx().content_rect().height() * ROWS_HEIGHT_SHARE;
    egui::ScrollArea::vertical()
        .max_height(room)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("labels-files")
                .num_columns(3)
                .spacing(egui::vec2(t.space_4, t.space_2))
                .show(ui, |ui| {
                    for heading in [
                        key!("labels-set"),
                        key!("labels-marker-part"),
                        key!("labels-count"),
                    ] {
                        ui.label(components::small_secondary(&t, strings::text(heading)).strong());
                    }
                    ui.end_row();
                    for f in files {
                        // The tick is named by its label set, which it stands beside.
                        let mut on = f.on;
                        if ui
                            .checkbox(&mut on, components::body_strong(&t, &f.set))
                            .changed()
                        {
                            events.push(Event::Action("labels.tick", json!({ "file": f.name, "on": on })));
                        }
                        ui.label(components::body(&t, &f.marker_part));
                        ui.label(components::body(&t, f.count.to_string()));
                        ui.end_row();
                    }
                });
        });
    if !d.misfits.is_empty() {
        ui.add_space(t.space_2);
        ui.horizontal(|ui| {
            ui.label(components::small_secondary(
                &t,
                strings::format_count(ew_viewmodel::count_key!("labels-misfits"), d.misfits.len()),
            ));
            let word = if d.show_misfits {
                key!("labels-hide")
            } else {
                key!("labels-show")
            };
            if ui.button(strings::text(word)).clicked() {
                events.push(Event::Action("labels.misfits", json!({ "on": !d.show_misfits })));
            }
        });
        if d.show_misfits {
            for m in &d.misfits {
                ui.label(components::body(&t, m));
            }
        }
    }
}
