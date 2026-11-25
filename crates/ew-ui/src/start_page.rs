// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start page (UX/UI doc 16 section 3.7): New project (its dialog says what the project starts from), Open, the
//! recent projects, Open sample project, Import QElectroTech project, Templates…, Tutorials, the quick switches for
//! language and keymap, and the privacy line "This version has no
//! update check" (nothing to change, so no button). Nothing on the page opens a connection (NFR-PRIV-01). It
//! appears before any project is touched; no work that could take time runs before the first frame.

use egui::Ui;
use ew_viewmodel::key;
use ew_viewmodel::settings::{KEYMAPS, LANGUAGES};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::Tip as _;

fn action(events: &mut Vec<Event>, id: &'static str, params: Value) {
    events.push(Event::Action(id, params));
}

/// A row of choices for one setting: the label, then one selectable button per option, centred in the page's column
/// as the rest of the page is.
fn quick_switch(
    ui: &mut Ui,
    events: &mut Vec<Event>,
    label_key: strings::Key<'_>,
    key: &str,
    current: &str,
    options: &[&str],
) {
    let label = strings::text(label_key);
    let texts: Vec<&str> = options
        .iter()
        .map(|option| ew_viewmodel::settings::choice_word(key, option))
        .collect();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let width_of = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), body.clone(), crate::theme::NONE)
            .size()
            .x
    };
    let spacing = ui.spacing();
    let width = width_of(label)
        + texts
            .iter()
            .map(|text| width_of(text) + 2.0 * spacing.button_padding.x + spacing.item_spacing.x)
            .sum::<f32>();
    let size = egui::vec2(width.min(ui.available_width()), spacing.interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.label(label);
        for (option, text) in options.iter().zip(texts) {
            if ui.selectable_label(*option == current, text).clicked() && *option != current {
                action(events, "settings.set", json!({ "key": key, "value": option }));
            }
        }
    });
}

/// Paints the start page into `ui`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = crate::theme::current(ui.ctx());
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * crate::theme::shares::START_TOP_SHARE).min(60.0));
            ui.heading(strings::text(key!("start-title")));
            ui.add_space(t.space_5);
            if ui.button(strings::text(key!("start-new"))).clicked() {
                action(events, "file.new", Value::Null);
            }
            ui.add_space(t.space_3);
            if ui.button(strings::text(key!("start-open"))).clicked() {
                action(events, "file.open", Value::Null);
            }
            let names = shell.recent_names();
            if !names.is_empty() {
                ui.add_space(t.space_4);
                ui.label(strings::text(key!("start-recent")));
                for (i, name) in names.iter().enumerate() {
                    let response = ui.button(name.as_str());
                    let response = match shell.recent_paths().get(i) {
                        Some(path) => response.tip(path.as_str()),
                        None => response,
                    };
                    if response.clicked() {
                        action(events, "file.open_recent", json!({ "index": i }));
                    }
                }
            }
            ui.add_space(t.space_4);
            let refusal =
                ew_viewmodel::actions::find("file.open_sample").and_then(|m| m.enabled_now(shell).err());
            let sample = ui.add_enabled(
                refusal.is_none(),
                egui::Button::new(strings::text(key!("start-sample"))),
            );
            if sample.clicked() {
                action(events, "file.open_sample", Value::Null);
            }
            if let Some(why) = &refusal {
                sample.disabled_tip(&strings::render(why));
            }
            if ui.button(strings::text(key!("start-import-qet"))).clicked() {
                action(events, "file.import_qet", Value::Null);
            }
            if ui.button(strings::text(key!("start-templates"))).clicked() {
                action(events, "templates.open", Value::Null);
            }
            // The tutorials and the how-to guides, as Help lists them (F5).
            ui.menu_button(strings::text(key!("start-tutorials")), |ui| {
                crate::menu::tutorial_lists(ui, shell, events);
            });
            ui.add_space(t.space_5);
            let settings = shell.settings();
            quick_switch(
                ui,
                events,
                key!("start-quick-language"),
                "language",
                &settings.language,
                &LANGUAGES,
            );
            quick_switch(
                ui,
                events,
                key!("start-quick-keymap"),
                "keymap",
                &settings.keymap,
                &KEYMAPS,
            );
            ui.add_space(t.space_5);
            // This version has no update check (doc 16a §8 item 2), so
            // there is nothing to change, and no Change button.
            ui.weak(strings::text(key!("start-update-check")));
        });
    });
}
