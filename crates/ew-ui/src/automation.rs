// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialog contents of the automation workspaces (WP-S2.13), drawn inside the one dialog frame
//! ([`crate::dialog`]) from the shell's views.
//!
//! **The rack and slot editor** (doc 10 §6, FR-PLC-028): the stored order codes of a valve terminal, the module part
//! to add (a combo box), then one row per slot: its number (a field on a module's row: typing a slot and Enter moves
//! the module), the module's tag, its part, the slots the part allows and, on a free slot, Add here. A valve
//! terminal's module shows its valve positions under its row. Below the rows EW-PLC-011's findings with the error icon
//! and word, else the line that the slots meet the station's rules; a refused change stays under them in the error
//! colour.

use egui::{RichText, ScrollArea, Ui};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::key;
use ew_viewmodel::shell::{ShellModel, rack_positions_text, rack_slot_text};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::icons::{self, IconId};
use crate::theme;

/// The rack and slot editor's content for `station`, with the last refusal `error`.
pub(crate) fn rack(
    ui: &mut Ui,
    shell: &ShellModel,
    station: Uuid,
    error: Option<&str>,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let Some(view) = shell.rack_view(station) else {
        return;
    };
    let small = |text: String| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32())
    };
    for (label, code) in &view.order_codes {
        ui.label(small(strings::format(key!("rack-order-code"), &[label, code])));
    }
    let parts = shell.module_parts();
    let chosen = shell.rack_part();
    ui.horizontal(|ui| {
        ui.label(small(strings::text(key!("rack-module-part")).to_owned()));
        let shown = chosen.as_ref().map(|p| p.text.clone()).unwrap_or_default();
        let combo = egui::ComboBox::from_id_salt("rack-part")
            .width(ui.available_width())
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for p in &parts {
                    let on = chosen.as_ref().is_some_and(|c| c.copy == p.copy);
                    if ui.selectable_label(on, &p.text).clicked() {
                        events.push(Event::Action(
                            "plc.rack_part",
                            json!({ "part": p.copy.to_string() }),
                        ));
                    }
                }
            });
        let name = strings::text(key!("rack-module-part")).to_owned();
        combo
            .response
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
        components::combo_expanded(ui, &combo.response);
    });
    let header = |text: &str| {
        RichText::new(text)
            .font(small_strong(&t))
            .color(t.text_secondary.color32())
    };
    ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.5)
        .show(ui, |ui| {
            egui::Grid::new("rack-slots")
                .num_columns(5)
                .spacing(egui::vec2(t.space_4, t.space_2))
                .striped(false)
                .show(ui, |ui| {
                    for k in [
                        key!("rack-slot"),
                        key!("rack-module"),
                        key!("rack-part"),
                        key!("rack-allowed"),
                    ] {
                        ui.label(header(strings::text(k)));
                    }
                    ui.label("");
                    ui.end_row();
                    for r in &view.rows {
                        let slot = rack_slot_text(r);
                        match r.module {
                            Some(module) => {
                                let id = egui::Id::new(("rack-slot", module));
                                let mut text = ui
                                    .data_mut(|d| d.get_temp::<String>(id))
                                    .unwrap_or_else(|| slot.clone());
                                let name = strings::format(key!("rack-slot-field"), &[&r.tag]);
                                let edit = ui.add(
                                    egui::TextEdit::singleline(&mut text)
                                        .desired_width(t.size_table_column_min)
                                        .font(theme::font(&t.type_small)),
                                );
                                ui.ctx()
                                    .accesskit_node_builder(edit.id, |n| n.set_label(name.clone()));
                                let enter =
                                    edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if enter && text.trim() != slot {
                                    events.push(Event::Action(
                                        "plc.rack_move",
                                        json!({ "module": module.to_string(), "slot": text.trim() }),
                                    ));
                                }
                                if edit.has_focus() {
                                    ui.data_mut(|d| d.insert_temp(id, text));
                                } else {
                                    ui.data_mut(|d| d.remove::<String>(id));
                                }
                                ui.label(RichText::new(&r.tag).color(t.text_primary.color32()));
                                ui.label(small(r.part.clone()));
                                allowed_cell(ui, &t, &r.allowed, r.slot);
                                ui.label("");
                            }
                            None => {
                                ui.label(small(slot));
                                ui.label(
                                    RichText::new(strings::text(key!("rack-free")))
                                        .font(theme::font(&t.type_small))
                                        .color(t.text_tertiary.color32()),
                                );
                                ui.label("");
                                match &chosen {
                                    Some(p) => allowed_cell(ui, &t, &p.allowed, r.slot),
                                    None => {
                                        ui.label("");
                                    }
                                }
                                let label = strings::text(key!("rack-add-here"));
                                let add =
                                    components::button(ui, Kind::Secondary, label, chosen.is_some(), 0.0);
                                let name = format!("{label} {}", r.slot);
                                add.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::Role::Button, chosen.is_some(), &name)
                                });
                                if add.clicked() {
                                    events.push(Event::Action(
                                        "plc.rack_add",
                                        json!({ "slot": r.slot, "rack": r.rack }),
                                    ));
                                }
                            }
                        }
                        ui.end_row();
                        if !r.positions.is_empty() {
                            ui.label("");
                            ui.label(small(strings::format(
                                key!("rack-positions"),
                                &[&rack_positions_text(r)],
                            )));
                            ui.end_row();
                        }
                    }
                });
        });
    // EW-PLC-011, live: each finding with the error icon and word, or the line that the slots meet the rules.
    let lang = strings::rule_language();
    if view.findings.is_empty() {
        ui.label(small(strings::text(key!("rack-no-findings")).to_owned()));
    }
    for f in &view.findings {
        let text = ew_viewmodel::ew_api::checks::messages::render(f, lang);
        severity_line(ui, &t, &text);
    }
    if let Some(e) = error {
        ui.label(
            RichText::new(e)
                .font(theme::font(&t.type_small))
                .color(t.error.color32()),
        );
    }
}

/// The Allowed slots cell: the part's slots (`any` without a rule); a slot the part does not allow marked with the
/// error icon and the slots in the error colour, never by colour alone.
fn allowed_cell(ui: &mut Ui, t: &theme::Tokens, allowed: &[u16], slot: u32) {
    let fits = allowed.is_empty() || allowed.iter().any(|a| u32::from(*a) == slot);
    let text = if allowed.is_empty() {
        strings::text(key!("rack-any-slot")).to_owned()
    } else {
        allowed.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
    };
    ui.horizontal(|ui| {
        if !fits {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(t.icon_sm, t.icon_sm), egui::Sense::hover());
            icons::paint(
                ui.painter(),
                IconId::circle_x,
                rect,
                t.error.color32(),
                t.icon_stroke,
            );
        }
        let colour = if fits { t.text_secondary } else { t.error };
        ui.label(
            RichText::new(text)
                .font(theme::font(&t.type_small))
                .color(colour.color32()),
        );
    });
}

/// `type.small` in SemiBold: the table headers and the severity word.
fn small_strong(t: &theme::Tokens) -> egui::FontId {
    theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    })
}

/// One EW-PLC-011 finding: the error icon, the word "Error" in SemiBold and the message (FR-UI-025).
fn severity_line(ui: &mut Ui, t: &theme::Tokens, text: &str) {
    ui.horizontal_wrapped(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t.icon_sm, t.icon_sm), egui::Sense::hover());
        icons::paint(
            ui.painter(),
            IconId::circle_x,
            rect,
            t.error.color32(),
            t.icon_stroke,
        );
        ui.label(
            RichText::new(strings::text(key!("severity-error")))
                .font(small_strong(t))
                .color(t.error.color32()),
        );
        ui.label(
            RichText::new(text)
                .font(theme::font(&t.type_small))
                .color(t.text_primary.color32()),
        );
    });
}
