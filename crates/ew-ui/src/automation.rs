// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dialog contents of the automation workspaces (WP-S2.13), drawn inside the one dialog frame
//! ([`crate::dialog`]) from the shell's views.
//!
//! **The rack and slot editor** (doc 10 §6, FR-PLC-028): the stored order codes of a valve terminal, the module part
//! to add (a combo box), then one row per slot: its number (a field on a module's row: typing a slot and Enter moves
//! the module; a module dragged by its tag onto another slot's row moves there too), the module's tag, its part, the
//! slots the part allows and, on a free slot, Add here. A valve terminal's module shows its valve positions under its
//! row. Below the rows EW-PLC-011's findings with the error icon
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
                                let id = egui::Id::unique(("rack-slot", module));
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
                                // The tag is the handle a module is dragged by, and a slot another one is dropped on.
                                let tag = ui
                                    .dnd_drag_source(egui::Id::unique(("rack-drag", module)), module, |ui| {
                                        ui.label(RichText::new(&r.tag).color(t.text_primary.color32()))
                                    })
                                    .response;
                                let handle = strings::format(key!("rack-drag"), &[&r.tag]);
                                ui.ctx()
                                    .accesskit_node_builder(tag.id, |n| n.set_label(handle.clone()));
                                drop_on(ui, &t, &tag, &slot, Some(module), events);
                                ui.label(small(r.part.clone()));
                                allowed_cell(ui, &t, &r.allowed, r.slot);
                                ui.label("");
                            }
                            None => {
                                ui.label(small(slot.clone()));
                                let free = ui.label(
                                    RichText::new(strings::text(key!("rack-free")))
                                        .font(theme::font(&t.type_small))
                                        .color(t.text_tertiary.color32()),
                                );
                                drop_on(ui, &t, &free, &slot, None, events);
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

/// A slot's module cell `cell` as a drop target (FT-1752): a module dragged by its tag and let go over the cell moves
/// to slot `slot` (`plc.rack_move`, as the slot field does); while it hovers, the cell is outlined in the accent colour.
fn drop_on(
    ui: &Ui,
    t: &theme::Tokens,
    cell: &egui::Response,
    slot: &str,
    own: Option<Uuid>,
    events: &mut Vec<Event>,
) {
    if cell.dnd_hover_payload::<Uuid>().is_some_and(|m| Some(*m) != own) {
        ui.painter().rect_stroke(
            cell.rect.expand(t.space_1),
            theme::radius(t.radius_sm),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(module) = cell.dnd_release_payload::<Uuid>()
        && Some(*module) != own
    {
        events.push(Event::Action(
            "plc.rack_move",
            json!({ "module": module.to_string(), "slot": slot }),
        ));
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

/// The PLC import's preview (doc 10 §6): one tab per list with its count (Changed, Conflicts, Removed, Parking), then
/// the shown tab's rows: a change's object and field with the project's and the file's value and what the merge makes
/// of it; a conflict's choice between keeping the project's value and taking the file's; a removed object; a parked
/// object, with Create where it can be created unplaced.
pub(crate) fn plc_import(
    ui: &mut Ui,
    card: &ew_viewmodel::shell::automation::PlcImportCard,
    events: &mut Vec<Event>,
) {
    use ew_viewmodel::shell::automation::ImportTab;
    let t = theme::current(ui.ctx());
    let small = |text: String, colour: theme::Rgba| {
        RichText::new(text)
            .font(theme::font(&t.type_small))
            .color(colour.color32())
    };
    ui.horizontal(|ui| {
        for tab in ImportTab::ALL {
            let label = strings::format(
                ew_viewmodel::strings::Key::run_time(&format!("plc-import-tab-{}", tab.word())),
                &[&card.count(tab).to_string()],
            );
            let on = card.tab == tab;
            let r = ui.selectable_label(on, label.clone());
            if r.clicked() {
                events.push(Event::Action("plc.import_tab", json!({ "tab": tab.word() })));
            }
        }
    });
    ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .auto_shrink([false, true])
        .show(ui, |ui| match card.tab {
            ImportTab::Changed | ImportTab::Conflicts => {
                let rows = if card.tab == ImportTab::Changed {
                    &card.changes
                } else {
                    &card.conflicts
                };
                for c in rows {
                    ui.label(small(c.text.clone(), t.text_primary));
                    ui.horizontal_wrapped(|ui| {
                        ui.label(small(c.ours.clone(), t.text_secondary));
                        ui.label(small("→".to_owned(), t.text_tertiary));
                        ui.label(small(c.theirs.clone(), t.text_primary));
                        let state = strings::text(ew_viewmodel::strings::Key::run_time(&format!(
                            "plc-import-state-{}",
                            c.state
                        )));
                        ui.label(small(format!("· {state}"), t.text_tertiary));
                    });
                    if card.tab == ImportTab::Conflicts {
                        // The side Apply takes: the one chosen, else the merge's default (FT-1756: the file's value
                        // only where it can be taken, else the reason why not).
                        let chosen = card.choices.get(&c.id).map_or(c.default, String::as_str);
                        ui.horizontal_wrapped(|ui| {
                            for (word, key) in [
                                ("ours", key!("plc-import-keep-ours")),
                                ("theirs", key!("plc-import-take-theirs")),
                            ] {
                                if word == "theirs"
                                    && let Some(why) = &c.refused
                                {
                                    ui.label(small(why.clone(), t.text_secondary));
                                    continue;
                                }
                                if ui.radio(chosen == word, strings::text(key)).clicked() {
                                    events.push(Event::Action(
                                        "plc.import_choose",
                                        json!({ "id": c.id, "choice": word }),
                                    ));
                                }
                            }
                        });
                    }
                    ui.add_space(t.space_2);
                }
            }
            ImportTab::Removed => {
                for r in &card.removed {
                    ui.label(small(r.clone(), t.text_primary));
                }
            }
            ImportTab::Parking => {
                for p in &card.parking {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(small(p.text.clone(), t.text_primary));
                        let why = if p.ambiguous {
                            key!("plc-import-ambiguous")
                        } else {
                            key!("plc-import-unmatched")
                        };
                        ui.label(small(format!("· {}", strings::text(why)), t.text_tertiary));
                        if p.creatable {
                            let mut create = card.choices.get(&p.id).is_some_and(|c| c == "create");
                            if ui
                                .checkbox(&mut create, strings::text(key!("plc-import-create")))
                                .changed()
                            {
                                let mut params = json!({ "id": p.id });
                                if create {
                                    params["choice"] = json!("create");
                                }
                                events.push(Event::Action("plc.import_choose", params));
                            }
                        }
                    });
                }
            }
        });
}
